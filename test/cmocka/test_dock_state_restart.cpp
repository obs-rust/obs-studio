/* #3740, #5230: the dock layout across window changes and restarts.
 * Exercise the production adapter (frontend/utility/DockLayout) with the
 * frontend's Rust model, Qt's layout serialization and the same config C API
 * / base64 / safe-save / reopen path as OBSBasic. */
#include <QApplication>
#include <QDockWidget>
#include <QLabel>
#include <QMainWindow>
#include <QPointer>
#include <QTabBar>
#include <QTemporaryDir>
#include <QTest>

#include "frontend/utility/DockLayout.hpp"
#include "frontend/utility/ExtraDocks.hpp"
#include <util/config-file.h>

#include <cstdio>
#include <map>
#include <optional>
#include <string>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

static void settle(QMainWindow &window)
{
	QList<QRect> last;
	int stable = 0;
	const bool settled = QTest::qWaitFor(
		[&] {
			QList<QRect> now{window.geometry()};
			for (QDockWidget *dock : window.findChildren<QDockWidget *>()) {
				now.append(dock->geometry());
			}
			stable = now == last ? stable + 1 : 0;
			last = now;
			return stable >= 3;
		},
		3000);
	assert_true(settled);
}

/* BasicWindow/DockSizes as the last save wrote it for each DockState blob:
 * the ini holds both keys, and the next start reads them together. */
static std::map<QByteArray, std::string> sizes_saved_with;

static const char *saved_sizes(const QByteArray &state)
{
	const auto it = sizes_saved_with.find(state);
	return it == sizes_saved_with.end() ? nullptr : it->second.c_str();
}

/* Real config_save_safe closes the file and renames the temporary file. Read
 * it back with a fresh config object, as the next process does. A blob saved
 * without sizes (upstream OBS, plain Qt) gets none on the next start. */
static QByteArray persist(const QByteArray &state, const std::string &sizes = {})
{
	QTemporaryDir dir(QStringLiteral("dock-restart-XXXXXX"));
	assert_true(dir.isValid());
	const QByteArray file = (dir.path() + QStringLiteral("/user.ini")).toUtf8();
	config_t *config = nullptr;
	assert_int_equal(config_open(&config, file.constData(), CONFIG_OPEN_ALWAYS), CONFIG_SUCCESS);
	config_set_string(config, "BasicWindow", "DockState", state.toBase64().constData());
	if (!sizes.empty()) {
		config_set_string(config, "BasicWindow", "DockSizes", sizes.c_str());
	}
	assert_int_equal(config_save_safe(config, "tmp", nullptr), CONFIG_SUCCESS);
	config_close(config);
	assert_int_equal(config_open(&config, file.constData(), CONFIG_OPEN_EXISTING), CONFIG_SUCCESS);
	const QByteArray reopened = QByteArray::fromBase64(config_get_string(config, "BasicWindow", "DockState"));
	const char *record = config_get_string(config, "BasicWindow", "DockSizes");
	if (record) {
		sizes_saved_with[reopened] = record;
	} else {
		sizes_saved_with.erase(reopened);
	}
	config_close(config);
	assert_true(reopened == state);
	return reopened;
}

struct Window {
	QMainWindow window;
	DockLayout layout{&window};
	QList<QDockWidget *> docks;
	/* What Reset Docks restores: OBSBasic gives it to
	 * DockLayout::SetDefault() before the user's layout is restored. */
	QByteArray startingLayout;

	Window()
	{
		window.setCentralWidget(new QLabel(QStringLiteral("Preview")));
		for (int i = 0; i < 5; ++i) {
			auto *dock = new QDockWidget(QString::number(i));
			dock->setObjectName(QStringLiteral("dock%1").arg(i));
			dock->setWidget(new QLabel(QStringLiteral("Contents")));
			dock->widget()->setMinimumSize(150, 80);
			window.addDockWidget(Qt::BottomDockWidgetArea, dock);
			if (!docks.isEmpty()) {
				window.splitDockWidget(docks.last(), dock, Qt::Horizontal);
			}
			docks.append(dock);
		}
		startingLayout = window.saveState();
		layout.SetDefault({docks[0], docks[1], docks[2], docks[3], docks[4]});
		window.resize(1600, 900);
	}

	void show()
	{
		window.show();
		settle(window);
	}

	/* OBSBasic::saveAll: DockState and DockSizes. */
	QByteArray save()
	{
		const QByteArray state = layout.StateToSave();
		return persist(state, layout.SizesRecord(state));
	}

	QList<QSize> sizes() const
	{
		QList<QSize> result;
		for (auto *dock : docks) {
			result.append(dock->size());
		}
		return result;
	}

	void restore(const QByteArray &state)
	{
		assert_true(window.restoreState(state));
		layout.Start(saved_sizes(state), state);
	}

	/* Drag the actual QMainWindow separator to make Controls as narrow as
	 * Qt allows. No direct changes to the helper's saved state. */
	void narrow_controls()
	{
		auto *controls = docks.last();
		const QPoint separator(controls->x() - 3, controls->y() + controls->height() / 2);
		const QPoint end = separator + QPoint(controls->width(), 0);
		QTest::mousePress(window.windowHandle(), Qt::LeftButton, Qt::NoModifier, separator);
		QTest::mouseMove(window.windowHandle(), end);
		QTest::mouseRelease(window.windowHandle(), Qt::LeftButton, Qt::NoModifier, end);
		settle(window);
		assert_int_equal(controls->width(), controls->minimumSizeHint().width());
	}

	void taller_docks()
	{
		const QPoint separator(docks.first()->width() / 2, docks.first()->y() - 3);
		const QPoint end = separator - QPoint(0, 150);
		const int before = docks.first()->height();
		QTest::mousePress(window.windowHandle(), Qt::LeftButton, Qt::NoModifier, separator);
		QTest::mouseMove(window.windowHandle(), end);
		QTest::mouseRelease(window.windowHandle(), Qt::LeftButton, Qt::NoModifier, end);
		settle(window);
		assert_true(docks.first()->height() > before + 100);
	}
};

static void assert_sizes(const QList<QSize> &expected, const QList<QSize> &actual)
{
	assert_int_equal(expected.size(), actual.size());
	for (qsizetype i = 0; i < expected.size(); ++i) {
		print_message("dock %d: saved %dx%d, restarted %dx%d\n", int(i), expected[i].width(),
			      expected[i].height(), actual[i].width(), actual[i].height());
		assert_in_range(actual[i].width(), expected[i].width() - 2, expected[i].width() + 2);
		assert_in_range(actual[i].height(), expected[i].height() - 2, expected[i].height() + 2);
	}
}

static void changed_layout_at_minimum_survives_restart(void **)
{
	QByteArray state;
	QList<QSize> expected;
	{
		Window first;
		first.show();
		first.window.resizeDocks(first.docks, {500, 200, 400, 200, 276}, Qt::Horizontal);
		settle(first.window);
		// Establish the older layout that the regression kept saving.
		first.restore(first.save());
		first.narrow_controls();
		first.taller_docks();
		expected = first.sizes();
		state = first.save();
	}
	QList<QSize> actual;
	{
		Window restarted;
		restarted.restore(state);
		restarted.show();
		actual = restarted.sizes();
	}
	assert_sizes(expected, actual);
}

/* A separator click in a narrow window resizes nothing: the next start at
 * full size shows the layout the user sized, not the clamp. */
static void separator_click_does_not_save_a_clamped_layout(void **)
{
	QByteArray state;
	QList<QSize> expected;
	{
		Window first;
		first.show();
		first.window.resizeDocks(first.docks, {500, 200, 400, 200, 276}, Qt::Horizontal);
		settle(first.window);
		first.restore(first.save());
		settle(first.window);
		expected = first.sizes();
		first.window.resize(700, 900);
		settle(first.window);
		const QPoint separator(first.docks.last()->x() - 3, first.docks.last()->y() + 40);
		QTest::mouseClick(first.window.windowHandle(), Qt::LeftButton, Qt::NoModifier, separator);
		settle(first.window);
		state = first.save();
	}
	QList<QSize> actual;
	{
		Window restarted;
		restarted.restore(state);
		restarted.show();
		actual = restarted.sizes();
	}
	assert_sizes(expected, actual);
}

static void changed_layout_after_clamped_start_survives_restart(void **)
{
	QByteArray state;
	{
		Window original;
		original.show();
		original.window.resizeDocks(original.docks, {500, 200, 400, 200, 276}, Qt::Horizontal);
		settle(original.window);
		state = original.save();
	}
	QList<QSize> expected;
	{
		Window first;
		first.window.resize(700, 900);
		first.restore(state);
		first.show();
		// No user edit: a clamped first session must preserve #2225's state.
		assert_true(first.save() == state);
		first.window.resize(1600, 900);
		settle(first.window);
		first.narrow_controls();
		expected = first.sizes();
		state = first.save();
	}
	QList<QSize> actual;
	{
		Window restarted;
		restarted.restore(state);
		restarted.show();
		actual = restarted.sizes();
	}
	assert_sizes(expected, actual);
}

/* OBSBasic shows the window before it restores the dock state. A tiling
 * window manager (Hyprland, #3740) then resizes the shown window to its tile:
 * the layout is restored into the start size and the window grows to the
 * size the layout was saved at. The next start must get the saved layout
 * back, not the layout as it was squeezed into the start size. */
static void layout_survives_window_manager_resize_after_restore(void **)
{
	QByteArray state;
	QList<QSize> expected;
	{
		Window original;
		original.show();
		original.window.resizeDocks(original.docks, {500, 200, 400, 200, 276}, Qt::Horizontal);
		settle(original.window);
		original.taller_docks();
		expected = original.sizes();
		state = original.save();
	}
	QList<QSize> tiled;
	{
		Window first;
		first.window.resize(1000, 600);
		first.show();
		first.restore(state);
		settle(first.window);
		first.window.resize(1600, 900);
		settle(first.window);
		tiled = first.sizes();
		state = first.save();
	}
	for (qsizetype i = 0; i < tiled.size(); ++i) {
		print_message("dock %d after the tile resize: %dx%d\n", int(i), tiled[i].width(), tiled[i].height());
	}
	QList<QSize> actual;
	{
		Window restarted;
		restarted.restore(state);
		restarted.show();
		actual = restarted.sizes();
	}
	assert_sizes(expected, actual);
}

/* The reporter's start from a terminal (#3740): the window first shares the
 * workspace with the terminal, too narrow for the layout, and gets the whole
 * workspace when the terminal is hidden. Every window resize before the user
 * changes a dock restores the layout again, not only the first one. */
static void layout_survives_several_window_manager_resizes(void **)
{
	QByteArray state;
	QList<QSize> expected;
	{
		Window original;
		original.show();
		original.window.resizeDocks(original.docks, {500, 200, 400, 200, 276}, Qt::Horizontal);
		settle(original.window);
		original.taller_docks();
		expected = original.sizes();
		state = original.save();
	}
	QList<QSize> full;
	{
		Window first;
		first.window.resize(1000, 600);
		first.show();
		first.restore(state);
		settle(first.window);
		first.window.resize(800, 900); // tiled next to the terminal
		settle(first.window);
		first.window.resize(1600, 900); // the terminal is hidden
		settle(first.window);
		full = first.sizes();
		state = first.save();
	}
	print_message("in the session, after the last resize:\n");
	assert_sizes(expected, full);
	QList<QSize> actual;
	{
		Window restarted;
		restarted.restore(state);
		restarted.show();
		actual = restarted.sizes();
	}
	print_message("after restart:\n");
	assert_sizes(expected, actual);
}

/* The reporter's full-screen start (#3740): the last session ended full
 * screen, so OBS starts full screen with the layout, then leaves full screen
 * into a smaller tile. Nobody touches a dock and OBS quits. The next start,
 * at full size again, must show the layout of the start, not the layout Qt
 * squeezed into the smaller window. */
static void layout_survives_a_session_at_a_smaller_size(void **)
{
	QByteArray state;
	QList<QSize> expected;
	{
		Window original;
		original.show();
		original.window.resizeDocks(original.docks, {500, 200, 400, 200, 276}, Qt::Horizontal);
		settle(original.window);
		original.taller_docks();
		expected = original.sizes();
		state = original.save();
	}
	{
		Window first;
		first.show(); // full size
		first.restore(state);
		settle(first.window);
		first.window.resize(1450, 800); // out of full screen, into a slightly smaller tile
		settle(first.window);
		print_message("in the session, at 1450x800:\n");
		for (QSize size : first.sizes()) {
			print_message("dock in session %dx%d\n", size.width(), size.height());
		}
		state = first.save();
	}
	QList<QSize> actual;
	{
		Window restarted;
		restarted.restore(state);
		restarted.show();
		actual = restarted.sizes();
	}
	print_message("after restart:\n");
	assert_sizes(expected, actual);
}

/* A dock the user hides through its menu action (View, Docks) stays hidden
 * when the window is resized afterwards and after a restart: restoring the
 * start layout on a window resize must not bring it back. */
static void hidden_dock_stays_hidden_after_a_window_resize(void **)
{
	QByteArray state;
	{
		Window original;
		original.show();
		state = original.save();
	}
	{
		Window first;
		first.window.resize(1000, 600);
		first.show();
		first.restore(state);
		settle(first.window);
		first.docks[1]->toggleViewAction()->trigger();
		settle(first.window);
		assert_false(first.docks[1]->isVisible());
		first.window.resize(1600, 900);
		settle(first.window);
		print_message("hidden dock after the window resize: visible=%d\n", first.docks[1]->isVisible());
		assert_false(first.docks[1]->isVisible());
		state = first.save();
	}
	{
		Window restarted;
		restarted.restore(state);
		restarted.show();
		print_message("hidden dock after restart: visible=%d\n", restarted.docks[1]->isVisible());
		assert_false(restarted.docks[1]->isVisible());
	}
}

/* A dock the user floats or moves to another area keeps that place across a
 * window resize and a restart, like a hidden one. */
static void moved_and_floated_docks_stay_after_a_window_resize(void **)
{
	QByteArray state;
	{
		Window original;
		original.show();
		state = original.save();
	}
	{
		Window first;
		first.window.resize(1000, 600);
		first.show();
		first.restore(state);
		settle(first.window);
		first.docks[0]->setFloating(true);
		settle(first.window);
		first.window.addDockWidget(Qt::LeftDockWidgetArea, first.docks[2]);
		settle(first.window);
		first.window.resize(1600, 900);
		settle(first.window);
		print_message("moved dock: floating=%d, area of dock2=%d\n", first.docks[0]->isFloating(),
			      int(first.window.dockWidgetArea(first.docks[2])));
		assert_true(first.docks[0]->isFloating());
		assert_int_equal(first.window.dockWidgetArea(first.docks[2]), Qt::LeftDockWidgetArea);
		state = first.save();
	}
	{
		Window restarted;
		restarted.restore(state);
		restarted.show();
		print_message("moved dock after restart: floating=%d, area of dock2=%d\n",
			      restarted.docks[0]->isFloating(),
			      int(restarted.window.dockWidgetArea(restarted.docks[2])));
		assert_true(restarted.docks[0]->isFloating());
		assert_int_equal(restarted.window.dockWidgetArea(restarted.docks[2]), Qt::LeftDockWidgetArea);
	}
}

/* A layout the user arranged: Controls in the left area, the other docks in
 * the bottom row at their own widths. */
static QByteArray arranged_layout()
{
	Window original;
	original.show();
	original.window.addDockWidget(Qt::LeftDockWidgetArea, original.docks[2]);
	original.window.resizeDocks({original.docks[0], original.docks[1], original.docks[3], original.docks[4]},
				    {500, 300, 300, 200}, Qt::Horizontal);
	settle(original.window);
	return original.save();
}

/* #5230: on a tiling window manager a dock change and a window resize often
 * come together (a dock dragged out of the window becomes a new window, and
 * the compositor retiles). The change is the user's: the resize must not put
 * the start layout back over it, and quit saves it. */
static void floated_dock_stays_when_the_window_resizes_at_the_same_time(void **)
{
	const QByteArray state = arranged_layout();
	{
		Window first;
		first.window.resize(1000, 600);
		first.show();
		first.restore(state);
		settle(first.window);
		first.docks[0]->setFloating(true);
		first.window.resize(1200, 700);
		settle(first.window);
		print_message("floated dock after the resize: floating=%d\n", first.docks[0]->isFloating());
		assert_true(first.docks[0]->isFloating());
		const QByteArray saved = first.save();
		Window restarted;
		restarted.restore(saved);
		restarted.show();
		print_message("floated dock after restart: floating=%d\n", restarted.docks[0]->isFloating());
		assert_true(restarted.docks[0]->isFloating());
	}
}

/* #5230: Reset Docks restores the default layout (DockLayout::Reset). When
 * the window resizes at the same time, the reset stays, and quit saves the
 * reset layout, not the one the session started with. */
static void reset_docks_stays_when_the_window_resizes_at_the_same_time(void **)
{
	const QByteArray state = arranged_layout();
	{
		Window first;
		first.window.resize(1000, 600);
		first.show();
		first.restore(state);
		settle(first.window);
		assert_int_equal(first.window.dockWidgetArea(first.docks[2]), Qt::LeftDockWidgetArea);
		first.layout.Reset();
		first.window.resize(1200, 700);
		settle(first.window);
		print_message("dock 2 area after reset and resize: %d\n",
			      int(first.window.dockWidgetArea(first.docks[2])));
		assert_int_equal(first.window.dockWidgetArea(first.docks[2]), Qt::BottomDockWidgetArea);
		const QByteArray saved = first.save();
		Window restarted;
		restarted.restore(saved);
		restarted.show();
		print_message("dock 2 area after restart: %d\n",
			      int(restarted.window.dockWidgetArea(restarted.docks[2])));
		assert_int_equal(restarted.window.dockWidgetArea(restarted.docks[2]), Qt::BottomDockWidgetArea);
	}
}

/* #5462: the main window as OBSBasic arranges its default layout: Scenes and
 * Sources on the left, Mixer, Transitions and Controls at the bottom, Stats a
 * hidden floating dock outside the default layout. */
struct DefaultWindow {
	QMainWindow window;
	DockLayout layout{&window};
	QDockWidget *scenes = dock("scenesDock");
	QDockWidget *sources = dock("sourcesDock");
	QDockWidget *mixer = dock("mixerDock");
	QDockWidget *transitions = dock("transitionsDock");
	QDockWidget *controls = dock("controlsDock");
	QDockWidget *stats = nullptr;

	QDockWidget *dock(const char *name)
	{
		auto *d = new QDockWidget(QString::fromLatin1(name));
		d->setObjectName(QString::fromLatin1(name));
		d->setWidget(new QLabel(QStringLiteral("Contents")));
		d->widget()->setMinimumSize(100, 80);
		return d;
	}

	DefaultWindow()
	{
		window.setCentralWidget(new QLabel(QStringLiteral("Preview")));
		/* The .ui file puts the four docks at the bottom. */
		for (QDockWidget *d : {scenes, sources, mixer, transitions}) {
			window.addDockWidget(Qt::BottomDockWidgetArea, d);
		}
		/* OBSBasic::OBSBasic, setDockCornersVertical(true). */
		window.setCorner(Qt::TopLeftCorner, Qt::LeftDockWidgetArea);
		window.setCorner(Qt::TopRightCorner, Qt::RightDockWidgetArea);
		window.setCorner(Qt::BottomLeftCorner, Qt::LeftDockWidgetArea);
		window.setCorner(Qt::BottomRightCorner, Qt::RightDockWidgetArea);
		window.addDockWidget(Qt::LeftDockWidgetArea, scenes);
		window.splitDockWidget(scenes, sources, Qt::Vertical);
		window.addDockWidget(Qt::BottomDockWidgetArea, controls);
		layout.SetDefault({scenes, sources, mixer, transitions, controls});
		stats = dock("statsDock");
		window.addDockWidget(Qt::BottomDockWidgetArea, stats);
		stats->setVisible(false);
		stats->setFloating(true);
		window.resize(1600, 900);
	}
};

/* #5462: Reset Docks after the user changed the layout gives the default
 * layout with the default sizes: side docks min(30 % of the width, 280 px),
 * bottom docks 22.5 % of the height, Mixer wider than Controls wider than
 * Transitions. Stats stays a hidden floating dock. The model starts with no
 * sizes and saves the reset layout. */
static void reset_docks_gives_the_default_layout_and_sizes(void **)
{
	Qt::DockWidgetArea scenesArea, sourcesArea, mixerArea;
	bool sourcesBelow, allShown, statsFloating, statsHidden, hasSizes, savesLive;
	int side, bottom, mixer, transitions, controls;
	{
		DefaultWindow w;
		w.window.show();
		settle(w.window);
		/* The user's layout: Scenes to the right, Transitions hidden,
		 * the bottom docks taller. */
		w.window.addDockWidget(Qt::RightDockWidgetArea, w.scenes);
		w.transitions->setVisible(false);
		w.window.resizeDocks({w.mixer}, {400}, Qt::Vertical);
		w.stats->setFloating(false);
		w.stats->setVisible(true);
		settle(w.window);
		/* OBSBasic::on_resetDocks_triggered puts Stats away first. */
		w.stats->setVisible(false);
		w.stats->setFloating(true);
		w.layout.Reset();
		settle(w.window);
		scenesArea = w.window.dockWidgetArea(w.scenes);
		sourcesArea = w.window.dockWidgetArea(w.sources);
		mixerArea = w.window.dockWidgetArea(w.mixer);
		sourcesBelow = w.sources->geometry().top() > w.scenes->geometry().top();
		allShown = w.scenes->isVisible() && w.sources->isVisible() && w.mixer->isVisible() &&
			   w.transitions->isVisible() && w.controls->isVisible();
		statsFloating = w.stats->isFloating();
		statsHidden = !w.stats->isVisible();
		side = w.scenes->width();
		bottom = w.mixer->height();
		mixer = w.mixer->width();
		transitions = w.transitions->width();
		controls = w.controls->width();
		hasSizes = w.layout.HasSizes();
		savesLive = w.layout.StateToSave() == w.window.saveState();
		print_message("after reset: side %d, bottom %d, mixer %d, transitions %d, controls %d\n", side, bottom,
			      mixer, transitions, controls);
	}
	assert_int_equal(scenesArea, Qt::LeftDockWidgetArea);
	assert_int_equal(sourcesArea, Qt::LeftDockWidgetArea);
	assert_int_equal(mixerArea, Qt::BottomDockWidgetArea);
	assert_true(sourcesBelow);
	assert_true(allShown);
	assert_true(statsFloating);
	assert_true(statsHidden);
	assert_in_range(side, 278, 282);
	assert_in_range(bottom, 900 * 225 / 1000 - 4, 900 * 225 / 1000 + 4);
	assert_true(mixer > controls && controls > transitions);
	assert_false(hasSizes);
	assert_true(savesLive);
}

/* #5462: the side docks get at most 30 % of a narrow window. */
static void reset_docks_in_a_narrow_window(void **)
{
	int side;
	{
		DefaultWindow w;
		w.window.resize(800, 700);
		w.window.show();
		settle(w.window);
		w.layout.Reset();
		settle(w.window);
		side = w.scenes->width();
		print_message("after reset at 800 px: side %d\n", side);
	}
	assert_in_range(side, 800 * 30 / 100 - 2, 800 * 30 / 100 + 2);
}

/* #5455: a config file as OBSBasic and the service logins use it, through
 * DockLayout::Save() and DockLayout::Load(). reopen() saves it safely and
 * opens it again, as the next process does. */
struct ConfigFile {
	QTemporaryDir dir{QStringLiteral("dock-config-XXXXXX")};
	QByteArray path;
	config_t *config = nullptr;

	ConfigFile()
	{
		assert_true(dir.isValid());
		path = (dir.path() + QStringLiteral("/user.ini")).toUtf8();
		assert_int_equal(config_open(&config, path.constData(), CONFIG_OPEN_ALWAYS), CONFIG_SUCCESS);
	}

	~ConfigFile() { config_close(config); }

	void reopen()
	{
		assert_int_equal(config_save_safe(config, "tmp", nullptr), CONFIG_SUCCESS);
		config_close(config);
		config = nullptr;
		assert_int_equal(config_open(&config, path.constData(), CONFIG_OPEN_EXISTING), CONFIG_SUCCESS);
	}
};

/* #5455: a service layout (Twitch, YouTube, Restream) saved the layout but
 * not its dock sizes, so each login started it with no sizes and the sizes
 * the user gave its docks were lost. Save() and Load() keep them for any
 * section. */
static void service_layout_keeps_its_sizes(void **)
{
	ConfigFile file;
	QList<QSize> expected;
	{
		Window first;
		first.show();
		first.narrow_controls();
		expected = first.sizes();
		first.layout.Save(file.config, "Twitch");
	}
	file.reopen();
	const bool has_state = config_get_string(file.config, "Twitch", "DockState") != nullptr;
	const bool has_sizes = config_get_string(file.config, "Twitch", "DockSizes") != nullptr;
	bool loaded = false;
	bool known = false;
	QList<QSize> actual;
	{
		Window next;
		loaded = next.layout.Load(file.config, "Twitch");
		known = next.layout.HasSizes();
		next.show();
		actual = next.sizes();
	}
	print_message("service section: DockState %d DockSizes %d; loaded %d with its sizes %d (expected 1 1 1 1)\n",
		      has_state, has_sizes, loaded, known);
	assert_true(has_state);
	assert_true(has_sizes);
	assert_true(loaded);
	assert_true(known);
	assert_sizes(expected, actual);
}

/* #5455: the start and quit (BasicWindow) through the same Load() and
 * Save(): a section without a layout loads nothing and leaves the window as
 * it was, so OBSBasic falls back to Reset Docks. */
static void main_layout_loads_and_saves_through_one_path(void **)
{
	ConfigFile file;
	bool empty_loaded = true;
	bool unchanged = false;
	{
		Window none;
		const QByteArray before = none.window.saveState();
		empty_loaded = none.layout.Load(file.config, "BasicWindow");
		unchanged = none.window.saveState() == before;
	}
	QList<QSize> expected;
	{
		Window first;
		first.show();
		first.narrow_controls();
		expected = first.sizes();
		first.layout.Save(file.config, "BasicWindow");
	}
	file.reopen();
	bool loaded = false;
	bool known = false;
	QList<QSize> actual;
	{
		Window next;
		loaded = next.layout.Load(file.config, "BasicWindow");
		known = next.layout.HasSizes();
		next.show();
		actual = next.sizes();
	}
	print_message("empty section loaded %d (expected 0); BasicWindow loaded %d with its sizes %d (expected 1 1)\n",
		      empty_loaded, loaded, known);
	assert_false(empty_loaded);
	assert_true(unchanged);
	assert_true(loaded);
	assert_true(known);
	assert_sizes(expected, actual);
}

/* Two window-manager profiles (#5230, W2). Every case below runs once under a
 * floating window manager and once under a tiling one, and both runs must
 * give the same layout after a restart. The code under test never knows
 * which profile runs (W1): a tiling window manager only sends more window
 * changes.
 *
 * - floating: the window changes size only when the case asks for it (the
 *   user drags the window edge).
 * - tiling: the window starts in a small tile and is retiled twice (#3740);
 *   a floated dock is a new window, so the main window gets half the tile
 *   at once (#5230); another window that opens or closes retiles it. */
enum class Wm { floating, tiling };

static const char *wm_name(Wm wm)
{
	return wm == Wm::floating ? "floating" : "tiling";
}

static const QSize full_size(1600, 900);

struct Session {
	Wm wm;
	Window w;
	/* Windows next to the main window in its tile: floated docks and
	 * other applications. */
	int others = 0;

	explicit Session(Wm wm) : wm(wm) {}

	QSize tile() const { return {full_size.width() / (others + 1), full_size.height()}; }

	void retile()
	{
		if (wm == Wm::tiling) {
			w.window.resize(tile());
		}
	}

	/* OBSBasic shows the window, then restores the dock state into it. */
	void start(const QByteArray &state)
	{
		if (wm == Wm::tiling) {
			w.window.resize(1000, 600);
		}
		w.show();
		if (!state.isEmpty()) {
			w.restore(state);
		}
		settle(w.window);
		/* Restored floating docks are windows of their own. */
		for (auto *dock : w.docks) {
			others += dock->isFloating() && dock->isVisible() ? 1 : 0;
		}
		if (wm == Wm::tiling) {
			w.window.resize(800, 900); // tiled next to the terminal
			settle(w.window);
			retile(); // the terminal is hidden
			settle(w.window);
		}
	}

	/* A window change: a retile, or the user drags the window edge on a
	 * floating window manager (D2: the same thing). */
	void window_change(QSize size)
	{
		w.window.resize(size);
		settle(w.window);
	}

	void other_window_opens()
	{
		++others;
		if (wm == Wm::tiling) {
			window_change(tile());
		}
	}

	void other_window_closes()
	{
		--others;
		if (wm == Wm::tiling) {
			window_change(tile());
		}
	}

	/* The floated dock is a new window: the tiling window manager retiles
	 * in the same event loop turn (#5230). */
	void float_dock(int i)
	{
		w.docks[i]->setFloating(true);
		++others;
		retile();
		settle(w.window);
	}

	/* The real Reset Docks path (OBSBasic::on_resetDocks_triggered). */
	void reset_docks()
	{
		w.layout.Reset();
		settle(w.window);
	}

	/* OBSBasic::saveAll writes what the adapter selects. */
	QByteArray quit() { return w.save(); }
};

struct Shown {
	Qt::DockWidgetArea area;
	bool floating;
	bool visible;
	QSize size;
};

static QList<Shown> capture(const Window &w)
{
	QList<Shown> result;
	for (auto *dock : w.docks) {
		result.append({w.window.dockWidgetArea(dock), dock->isFloating(), dock->isVisible(), dock->size()});
	}
	return result;
}

static void print_layout(const char *what, const QList<Shown> &layout)
{
	for (qsizetype i = 0; i < layout.size(); ++i) {
		print_message("%s dock %d: area=%d floating=%d visible=%d %dx%d\n", what, int(i), int(layout[i].area),
			      layout[i].floating, layout[i].visible, layout[i].size.width(), layout[i].size.height());
	}
}

/* Same placement and visibility; sizes within 2 px for docks on screen,
 * unless the case compares placement only. */
static void assert_layout(const QList<Shown> &expected, const QList<Shown> &actual, bool sizes = true)
{
	print_layout("expected", expected);
	print_layout("actual", actual);
	assert_int_equal(expected.size(), actual.size());
	for (qsizetype i = 0; i < expected.size(); ++i) {
		assert_int_equal(expected[i].area, actual[i].area);
		assert_int_equal(expected[i].floating, actual[i].floating);
		assert_int_equal(expected[i].visible, actual[i].visible);
		if (sizes && actual[i].visible && !actual[i].floating) {
			assert_in_range(actual[i].size.width(), expected[i].size.width() - 2,
					expected[i].size.width() + 2);
			assert_in_range(actual[i].size.height(), expected[i].size.height() - 2,
					expected[i].size.height() + 2);
		}
	}
}

/* The next start under the same window manager: the layout the user gets. */
static QList<Shown> restart(Wm wm, const QByteArray &saved)
{
	Session next(wm);
	next.start(saved);
	return capture(next.w);
}

/* A saved layout as a floating window manager shows it at full size: the
 * same reference window for every profile. */
static QList<Shown> arranged_at_full_size(const QByteArray &state)
{
	Session reference(Wm::floating);
	reference.start(state);
	return capture(reference.w);
}

/* A case gives the layout the user expects, the layout after a restart under
 * the same window manager, and what quit saved. Each profile must give what
 * the user expects, and both must save the same layout (W2). */
struct Outcome {
	QList<Shown> expected;
	QList<Shown> restarted;
	QByteArray saved;
};

static void run_both(Outcome (*run)(Wm), bool sizes = true)
{
	QList<Shown> saved[2];
	for (Wm wm : {Wm::floating, Wm::tiling}) {
		print_message("== %s window manager\n", wm_name(wm));
		const Outcome outcome = run(wm);
		assert_layout(outcome.expected, outcome.restarted, sizes);
		saved[int(wm)] = arranged_at_full_size(outcome.saved);
	}
	print_message("== saved layout, floating vs tiling\n");
	assert_layout(saved[0], saved[1], sizes);
}

/* E1, E4: start, quit without a change, start again. Under the tiling
 * profile the start is in a small tile and retiled twice (#3740). */
static Outcome e1_unchanged_layout(Wm wm)
{
	const QByteArray state = arranged_layout();
	QByteArray saved;
	{
		Session session(wm);
		session.start(state);
		saved = session.quit();
	}
	return {arranged_at_full_size(state), restart(wm, saved), saved};
}

/* E2: the window gets narrow and wide again in the same session (#2225):
 * the docks get their sizes back, and quit saves them. */
static Outcome e2_narrow_then_wide(Wm wm)
{
	const QByteArray state = arranged_layout();
	QByteArray saved;
	QList<Shown> in_session;
	{
		Session session(wm);
		session.start(state);
		if (wm == Wm::tiling) {
			session.other_window_opens();
			session.other_window_closes();
		} else {
			session.window_change({700, 900});
			session.window_change(full_size);
		}
		in_session = capture(session.w);
		saved = session.quit();
	}
	const QList<Shown> expected = arranged_at_full_size(state);
	print_message("in the session, wide again:\n");
	assert_layout(expected, in_session);
	return {expected, restart(wm, saved), saved};
}

/* E3: quit while the window is narrow, start at full size. */
static Outcome e3_quit_narrow(Wm wm)
{
	const QByteArray state = arranged_layout();
	QByteArray saved;
	{
		Session session(wm);
		session.start(state);
		if (wm == Wm::tiling) {
			session.other_window_opens();
		} else {
			session.window_change({700, 900});
		}
		saved = session.quit();
	}
	return {arranged_at_full_size(state), restart(wm, saved), saved};
}

/* E9: the user floats a dock; a tiling window manager retiles at once
 * (#5230). The dock stays floating, the other docks keep their place.
 * Placement only: under the tiling profile quit saves the row as squeezed
 * into the half tile, and the next full-size start gives one dock all the
 * extra width (P7, a RED case of the rewrite). */
static Outcome e9_float_a_dock(Wm wm)
{
	QByteArray saved;
	QList<Shown> expected;
	{
		Session session(wm);
		session.start(arranged_layout());
		session.float_dock(0);
		expected = capture(session.w);
		saved = session.quit();
	}
	return {expected, restart(wm, saved), saved};
}

/* E10: the user drags Controls to its minimum on purpose (#3740). */
static Outcome e10_dock_at_minimum(Wm wm)
{
	QByteArray saved;
	QList<Shown> expected;
	{
		Session session(wm);
		session.start(arranged_layout());
		session.w.narrow_controls();
		expected = capture(session.w);
		saved = session.quit();
	}
	return {expected, restart(wm, saved), saved};
}

/* E11: the user hides every dock for a preview-only window (F3). */
static Outcome e11_all_docks_hidden(Wm wm)
{
	QByteArray saved;
	QList<Shown> expected;
	{
		Session session(wm);
		session.start(arranged_layout());
		for (auto *dock : session.w.docks) {
			dock->toggleViewAction()->trigger();
		}
		settle(session.w.window);
		session.other_window_opens();
		session.other_window_closes();
		expected = capture(session.w);
		saved = session.quit();
	}
	return {expected, restart(wm, saved), saved};
}

/* The dock whose tab is the current one in the window's tab group. */
static int current_tab(const Window &w)
{
	for (QTabBar *bar : w.window.findChildren<QTabBar *>()) {
		if (bar->count() < 2) {
			continue;
		}
		const QString title = bar->tabText(bar->currentIndex());
		for (qsizetype i = 0; i < w.docks.size(); ++i) {
			if (w.docks[i]->windowTitle() == title) {
				return int(i);
			}
		}
	}
	return -1;
}

/* E13: the user clicks a tab in a tab group; the current tab is kept. */
static void e13_current_tab(void **)
{
	int restarted_tab[2];
	for (Wm wm : {Wm::floating, Wm::tiling}) {
		print_message("== %s window manager\n", wm_name(wm));
		QByteArray state;
		{
			Session arrange(Wm::floating);
			arrange.start(QByteArray());
			arrange.w.window.tabifyDockWidget(arrange.w.docks[3], arrange.w.docks[4]);
			settle(arrange.w.window);
			state = arrange.quit();
		}
		QByteArray saved;
		{
			Session session(wm);
			session.start(state);
			session.w.docks[3]->raise();
			settle(session.w.window);
			print_message("current tab after the click: %d\n", current_tab(session.w));
			assert_int_equal(current_tab(session.w), 3);
			session.other_window_opens();
			session.other_window_closes();
			print_message("current tab after the retile: %d\n", current_tab(session.w));
			assert_int_equal(current_tab(session.w), 3);
			saved = session.quit();
		}
		Session next(wm);
		next.start(saved);
		restarted_tab[int(wm)] = current_tab(next.w);
		print_message("current tab after restart: %d\n", restarted_tab[int(wm)]);
		assert_int_equal(restarted_tab[int(wm)], 3);
	}
}

/* E23: OBS is hidden to the tray when it quits; saveAll runs while the
 * window is hidden. */
static Outcome e23_quit_hidden(Wm wm)
{
	const QByteArray state = arranged_layout();
	QByteArray saved;
	{
		Session session(wm);
		session.start(state);
		session.w.window.hide();
		QTest::qWait(50);
		saved = session.quit();
	}
	return {arranged_at_full_size(state), restart(wm, saved), saved};
}

/* E25: a layout written by plain Qt (upstream OBS) comes back unchanged
 * after a session without a change, and the layout obs-rust saves restores
 * the same in plain Qt (upstream OBS reads it back). */
static Outcome e25_upstream_round_trip(Wm wm)
{
	QByteArray upstream;
	{
		Window plain;
		plain.show();
		plain.window.addDockWidget(Qt::LeftDockWidgetArea, plain.docks[2]);
		plain.window.resizeDocks({plain.docks[0], plain.docks[1], plain.docks[3], plain.docks[4]},
					 {500, 300, 300, 200}, Qt::Horizontal);
		settle(plain.window);
		upstream = persist(plain.window.saveState());
	}
	QByteArray saved;
	{
		Session session(wm);
		session.start(upstream);
		saved = session.quit();
	}
	Window plain;
	assert_true(plain.window.restoreState(saved));
	plain.show();
	return {arranged_at_full_size(upstream), capture(plain), saved};
}

/* E14: a plugin adds its dock after OBSBasic restored the layout
 * (OBSBasic::AddDockWidget calls addDockWidget into the plugin's area). The
 * saved layout has that dock in the left area. Both profiles put it at the
 * same place, in the session and after a restart, and a retile does not move
 * it (the current helper restores the start layout again on a retile). */
static void e14_plugin_dock_after_restore(void **)
{
	QByteArray state;
	{
		Window original;
		auto *plugin = new QDockWidget(QStringLiteral("Plugin"));
		plugin->setObjectName(QStringLiteral("plugin"));
		plugin->setWidget(new QLabel(QStringLiteral("Contents")));
		original.window.addDockWidget(Qt::LeftDockWidgetArea, plugin);
		original.show();
		state = original.save();
	}
	Qt::DockWidgetArea in_session[2], restarted[2];
	for (Wm wm : {Wm::floating, Wm::tiling}) {
		QByteArray saved;
		{
			Session session(wm);
			if (wm == Wm::tiling) {
				session.w.window.resize(1000, 600);
			}
			session.w.show();
			session.w.restore(state);
			auto *plugin = new QDockWidget(QStringLiteral("Plugin"));
			plugin->setObjectName(QStringLiteral("plugin"));
			plugin->setWidget(new QLabel(QStringLiteral("Contents")));
			session.w.window.addDockWidget(Qt::RightDockWidgetArea, plugin);
			settle(session.w.window);
			if (wm == Wm::tiling) {
				session.window_change({800, 900});
				session.window_change(full_size);
			}
			in_session[int(wm)] = session.w.window.dockWidgetArea(plugin);
			saved = session.quit();
		}
		Window next;
		next.show();
		next.restore(saved);
		auto *plugin = new QDockWidget(QStringLiteral("Plugin"));
		plugin->setObjectName(QStringLiteral("plugin"));
		plugin->setWidget(new QLabel(QStringLiteral("Contents")));
		next.window.addDockWidget(Qt::RightDockWidgetArea, plugin);
		settle(next.window);
		restarted[int(wm)] = next.window.dockWidgetArea(plugin);
		print_message("%s: plugin dock area in the session %d, after restart %d\n", wm_name(wm),
			      int(in_session[int(wm)]), int(restarted[int(wm)]));
	}
	/* As upstream: addDockWidget puts a late dock into the plugin's area,
	 * not into its saved place. */
	for (int wm = 0; wm < 2; ++wm) {
		assert_int_equal(in_session[wm], Qt::RightDockWidgetArea);
		assert_int_equal(restarted[wm], Qt::RightDockWidgetArea);
	}
}

/* A reference: the arranged layout at full size under a floating window
 * manager, after `change` made the user's change. */
static QList<Shown> arranged_then(void (*change)(Session &))
{
	Session reference(Wm::floating);
	reference.start(arranged_layout());
	change(reference);
	settle(reference.w.window);
	return capture(reference.w);
}

static void move_controls_to_top(Session &session)
{
	session.w.window.addDockWidget(Qt::TopDockWidgetArea, session.w.docks[4]);
	settle(session.w.window);
}

/* P2 (E5): the window is small when the user moves a dock, and OBS quits
 * while it is small. The move is the user's change: it is saved. */
static Outcome e5_move_a_dock_small(Wm wm)
{
	QByteArray saved;
	{
		Session session(wm);
		session.start(arranged_layout());
		session.window_change({900, 700});
		move_controls_to_top(session);
		saved = session.quit();
	}
	return {arranged_then(move_controls_to_top), restart(wm, saved), saved};
}

/* P4 (E6): the user drags a separator, the window shrinks, the user moves a
 * dock, OBS quits small. The move and the dragged height are both kept. */
static Outcome e6_drag_shrink_move(Wm wm)
{
	QByteArray saved;
	int dragged = 0;
	{
		Session session(wm);
		session.start(arranged_layout());
		session.window_change(full_size);
		session.w.taller_docks();
		dragged = session.w.docks[0]->height();
		session.window_change({900, 700});
		move_controls_to_top(session);
		saved = session.quit();
	}
	const QList<Shown> restarted = restart(wm, saved);
	print_message("dragged height %d, after restart %d\n", dragged, restarted[0].size.height());
	assert_in_range(restarted[0].size.height(), dragged - 2, dragged + 2);
	return {arranged_then(move_controls_to_top), restarted, saved};
}

/* P5 (E7): Reset Docks in a small window, then quit. The reset layout is
 * saved, not the layout of the start. */
static Outcome e7_reset_docks_small(Wm wm)
{
	QByteArray saved;
	QByteArray reset;
	{
		Session session(wm);
		session.start(arranged_layout());
		session.window_change({900, 700});
		session.reset_docks();
		reset = session.w.startingLayout;
		saved = session.quit();
	}
	return {arranged_at_full_size(reset), restart(wm, saved), saved};
}

/* P6 (E8): the user moves a floating dock, then the window changes size.
 * The floating dock stays where the user put it, now and after a restart
 * (X11, Windows, macOS: Wayland gives a window no position, E21). */
static void e8_move_a_floating_dock(void **)
{
	for (Wm wm : {Wm::floating, Wm::tiling}) {
		print_message("== %s window manager\n", wm_name(wm));
		QByteArray saved;
		QPoint placed;
		{
			Session session(wm);
			session.start(arranged_layout());
			session.float_dock(0);
			/* Fully on screen: a restore keeps a floating dock on the
			 * screen (Qt), and the screens of the test hosts differ. */
			session.w.docks[0]->move(QPoint(120, 140));
			settle(session.w.window);
			placed = session.w.docks[0]->pos();
			session.window_change({1200, 800});
			print_message("floating dock at %d,%d, after the window change %d,%d\n", placed.x(), placed.y(),
				      session.w.docks[0]->x(), session.w.docks[0]->y());
			assert_true(session.w.docks[0]->isFloating());
			assert_true(session.w.docks[0]->pos() == placed);
			saved = session.quit();
		}
		Session next(wm);
		next.start(saved);
		print_message("after restart %d,%d\n", next.w.docks[0]->x(), next.w.docks[0]->y());
		assert_true(next.w.docks[0]->isFloating());
		assert_true(next.w.docks[0]->pos() == placed);
	}
}

/* P7: floating a dock retiles the window at once under a tiling window
 * manager. The row keeps its shares in the half tile, and the layout saved
 * is the one a floating window manager saves (run_both compares them), not
 * the row as squeezed into the half tile. */
static Outcome p7_float_a_dock_sizes(Wm wm)
{
	QByteArray saved;
	QList<Shown> expected;
	{
		Session session(wm);
		session.start(arranged_layout());
		session.float_dock(0);
		expected = capture(session.w);
		saved = session.quit();
	}
	return {expected, restart(wm, saved), saved};
}

static void e1_unchanged_layout_both(void **)
{
	run_both(e1_unchanged_layout);
}

static void e2_narrow_then_wide_both(void **)
{
	run_both(e2_narrow_then_wide);
}

static void e3_quit_narrow_both(void **)
{
	run_both(e3_quit_narrow);
}

static void e9_float_a_dock_both(void **)
{
	run_both(e9_float_a_dock, false);
}

static void e10_dock_at_minimum_both(void **)
{
	run_both(e10_dock_at_minimum);
}

static void e11_all_docks_hidden_both(void **)
{
	run_both(e11_all_docks_hidden);
}

static void e23_quit_hidden_both(void **)
{
	run_both(e23_quit_hidden);
}

static void e25_upstream_round_trip_both(void **)
{
	run_both(e25_upstream_round_trip);
}

static void e5_move_a_dock_small_both(void **)
{
	run_both(e5_move_a_dock_small, false);
}

static void e6_drag_shrink_move_both(void **)
{
	run_both(e6_drag_shrink_move, false);
}

static void e7_reset_docks_small_both(void **)
{
	run_both(e7_reset_docks_small, false);
}

static void p7_float_a_dock_sizes_both(void **)
{
	run_both(p7_float_a_dock_sizes);
}

/* #5471: the features of every dock kind come from DockFeatures(). */
static void dock_features_follow_lock_and_closable(void **)
{
	const auto all = QDockWidget::DockWidgetClosable | QDockWidget::DockWidgetMovable |
			 QDockWidget::DockWidgetFloatable;
	const auto noClose = QDockWidget::DockWidgetMovable | QDockWidget::DockWidgetFloatable;
	assert_true(DockFeatures(false) == all);
	assert_true(DockFeatures(false, true) == all);
	assert_true(DockFeatures(false, false) == noClose);
	assert_true(DockFeatures(true) == QDockWidget::NoDockWidgetFeatures);
	assert_true(DockFeatures(true, false) == QDockWidget::NoDockWidgetFeatures);
}

static QDockWidget *add_named_dock(QMainWindow &window, const char *name)
{
	auto *dock = new QDockWidget(QString::fromUtf8(name));
	dock->setObjectName(QString::fromUtf8(name));
	dock->setWidget(new QLabel(QString::fromUtf8(name)));
	window.addDockWidget(Qt::RightDockWidgetArea, dock);
	return dock;
}

/* #5471: Lock Docks reaches plugin and custom docks, and a dock added while
 * the docks are locked gets the locked features. */
static void extra_docks_follow_lock_docks(void **)
{
	const auto all = DockFeatures(false);
	const auto none = QDockWidget::NoDockWidgetFeatures;
	bool addedUnlocked, addedLocked, unlockedBoth, lockedBoth, listed, named, empty;
	{
		QMainWindow window;
		ExtraDocks docks;
		QDockWidget *plugin = add_named_dock(window, "pluginDock");
		QDockWidget *custom = add_named_dock(window, "customDock");
		plugin->setFeatures(none);
		custom->setFeatures(all);
		docks.Add(plugin, ExtraDocks::Kind::Plugin, false);
		docks.Add(custom, ExtraDocks::Kind::Custom, true);
		addedUnlocked = plugin->features() == all;
		addedLocked = custom->features() == none;
		docks.SetLocked(false);
		unlockedBoth = plugin->features() == all && custom->features() == all;
		docks.SetLocked(true);
		lockedBoth = plugin->features() == none && custom->features() == none;
		listed = docks.Docks(ExtraDocks::Kind::Plugin) == QList<QDockWidget *>{plugin} &&
			 docks.Docks(ExtraDocks::Kind::Custom) == QList<QDockWidget *>{custom};
		named = docks.Contains("pluginDock") && docks.Contains("customDock") && !docks.Contains("statsDock");
		empty = docks.IsEmpty();
	}
	assert_true(addedUnlocked);
	assert_true(addedLocked);
	assert_true(unlockedBoth);
	assert_true(lockedBoth);
	assert_true(listed);
	assert_true(named);
	assert_false(empty);
}

/* #5471: OBS deletes a removed plugin dock; a removed custom dock stays the
 * plugin's, only out of the window. */
static void extra_docks_remove_by_kind(void **)
{
	bool removedPlugin, pluginDeleted, removedCustom, customAlive, customOut, unknown, gone, empty;
	{
		QMainWindow window;
		ExtraDocks docks;
		QPointer<QDockWidget> plugin = add_named_dock(window, "pluginDock");
		QPointer<QDockWidget> custom = add_named_dock(window, "customDock");
		docks.Add(plugin, ExtraDocks::Kind::Plugin, false);
		docks.Add(custom, ExtraDocks::Kind::Custom, false);
		window.show();
		removedPlugin = docks.Remove(&window, "pluginDock");
		pluginDeleted = plugin.isNull();
		removedCustom = docks.Remove(&window, "customDock");
		customAlive = !custom.isNull();
		customOut = customAlive && window.dockWidgetArea(custom) == Qt::NoDockWidgetArea;
		unknown = docks.Remove(&window, "otherDock");
		gone = !docks.Contains("pluginDock") && !docks.Contains("customDock");
		empty = docks.IsEmpty();
	}
	assert_true(removedPlugin);
	assert_true(pluginDeleted);
	assert_true(removedCustom);
	assert_true(customAlive);
	assert_true(customOut);
	assert_false(unknown);
	assert_true(gone);
	assert_true(empty);
}

/* #5471: a custom dock keeps the name it was added with (OBS restores it when
 * the plugin renames the dock); a plugin dock has no such name. */
static void extra_docks_keep_the_custom_name(void **)
{
	std::optional<QString> customName, pluginName;
	bool stillNamed;
	{
		QMainWindow window;
		ExtraDocks docks;
		QDockWidget *plugin = add_named_dock(window, "pluginDock");
		QDockWidget *custom = add_named_dock(window, "customDock");
		docks.Add(plugin, ExtraDocks::Kind::Plugin, false);
		docks.Add(custom, ExtraDocks::Kind::Custom, false);
		custom->setObjectName("renamedDock");
		customName = docks.CustomName(custom);
		pluginName = docks.CustomName(plugin);
		stillNamed = docks.Contains("customDock") && !docks.Contains("renamedDock");
	}
	assert_true(customName.has_value() && *customName == "customDock");
	assert_false(pluginName.has_value());
	assert_true(stillNamed);
}

int main(int argc, char **argv)
{
#ifndef OBS_FRONTEND_RUST
	/* Without the frontend's Rust the adapter is upstream OBS: these cases
	 * would test nothing of #5230. */
	(void)argc;
	(void)argv;
	fprintf(stderr, "test_dock_state_restart needs the frontend's Rust (obs_frontend-static)\n");
	return 1;
#else
	QApplication app(argc, argv);
	const CMUnitTest tests[] = {
		cmocka_unit_test(changed_layout_at_minimum_survives_restart),
		cmocka_unit_test(changed_layout_after_clamped_start_survives_restart),
		cmocka_unit_test(separator_click_does_not_save_a_clamped_layout),
		cmocka_unit_test(layout_survives_window_manager_resize_after_restore),
		cmocka_unit_test(layout_survives_several_window_manager_resizes),
		cmocka_unit_test(layout_survives_a_session_at_a_smaller_size),
		cmocka_unit_test(hidden_dock_stays_hidden_after_a_window_resize),
		cmocka_unit_test(moved_and_floated_docks_stay_after_a_window_resize),
		cmocka_unit_test(floated_dock_stays_when_the_window_resizes_at_the_same_time),
		cmocka_unit_test(reset_docks_stays_when_the_window_resizes_at_the_same_time),
		cmocka_unit_test(reset_docks_gives_the_default_layout_and_sizes),
		cmocka_unit_test(reset_docks_in_a_narrow_window),
		cmocka_unit_test(dock_features_follow_lock_and_closable),
		cmocka_unit_test(extra_docks_follow_lock_docks),
		cmocka_unit_test(extra_docks_remove_by_kind),
		cmocka_unit_test(extra_docks_keep_the_custom_name),
		cmocka_unit_test(service_layout_keeps_its_sizes),
		cmocka_unit_test(main_layout_loads_and_saves_through_one_path),
		cmocka_unit_test(e1_unchanged_layout_both),
		cmocka_unit_test(e2_narrow_then_wide_both),
		cmocka_unit_test(e3_quit_narrow_both),
		cmocka_unit_test(e9_float_a_dock_both),
		cmocka_unit_test(e10_dock_at_minimum_both),
		cmocka_unit_test(e11_all_docks_hidden_both),
		cmocka_unit_test(e13_current_tab),
		cmocka_unit_test(e14_plugin_dock_after_restore),
		cmocka_unit_test(e23_quit_hidden_both),
		cmocka_unit_test(e25_upstream_round_trip_both),
		cmocka_unit_test(e5_move_a_dock_small_both),
		cmocka_unit_test(e6_drag_shrink_move_both),
		cmocka_unit_test(e7_reset_docks_small_both),
		cmocka_unit_test(e8_move_a_floating_dock),
		cmocka_unit_test(p7_float_a_dock_sizes_both),
	};
	return cmocka_run_group_tests(tests, nullptr, nullptr);
#endif
}
