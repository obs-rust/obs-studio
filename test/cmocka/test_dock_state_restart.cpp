/* #3740: a user can deliberately leave a dock at its minimum width.
 * Exercise the production dock-state selector, Qt's layout serialization and
 * the same config C API / base64 / safe-save / reopen path as OBSBasic. */
#include <QApplication>
#include <QDockWidget>
#include <QLabel>
#include <QMainWindow>
#include <QTabBar>
#include <QTemporaryDir>
#include <QTest>

#include "dock-proportions.hpp"
#include <util/config-file.h>

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

struct Window {
	QMainWindow window;
	DockProportions proportions{&window};
	QList<QDockWidget *> docks;
	/* What Reset Docks restores: OBSBasic saves it before the user's
	 * layout is restored (startingDockLayout). */
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
		window.resize(1600, 900);
	}

	void show()
	{
		window.show();
		settle(window);
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
		proportions.SetRestoredState(state);
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

/* Real config_save_safe closes the file and renames the temporary file. Read
 * it back with a fresh config object, as the next process does. */
static QByteArray persist(const QByteArray &state)
{
	QTemporaryDir dir(QStringLiteral("dock-restart-XXXXXX"));
	assert_true(dir.isValid());
	const QByteArray file = (dir.path() + QStringLiteral("/user.ini")).toUtf8();
	config_t *config = nullptr;
	assert_int_equal(config_open(&config, file.constData(), CONFIG_OPEN_ALWAYS), CONFIG_SUCCESS);
	config_set_string(config, "BasicWindow", "DockState", state.toBase64().constData());
	assert_int_equal(config_save_safe(config, "tmp", nullptr), CONFIG_SUCCESS);
	config_close(config);
	assert_int_equal(config_open(&config, file.constData(), CONFIG_OPEN_EXISTING), CONFIG_SUCCESS);
	const QByteArray reopened = QByteArray::fromBase64(config_get_string(config, "BasicWindow", "DockState"));
	config_close(config);
	assert_true(reopened == state);
	return reopened;
}

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
		first.restore(persist(first.proportions.StateToSave()));
		first.narrow_controls();
		first.taller_docks();
		expected = first.sizes();
		state = persist(first.proportions.StateToSave());
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

static void separator_click_does_not_save_a_clamped_layout(void **)
{
	QByteArray expected, actual;
	{
		Window first;
		first.show();
		first.window.resizeDocks(first.docks, {500, 200, 400, 200, 276}, Qt::Horizontal);
		settle(first.window);
		expected = persist(first.proportions.StateToSave());
		first.window.resize(700, 900);
		settle(first.window);
		const QPoint separator(first.docks.last()->x() - 3, first.docks.last()->y() + 40);
		QTest::mouseClick(first.window.windowHandle(), Qt::LeftButton, Qt::NoModifier, separator);
		settle(first.window);
		actual = persist(first.proportions.StateToSave());
	}
	assert_true(actual == expected);
}

static void changed_layout_after_clamped_start_survives_restart(void **)
{
	QByteArray state;
	{
		Window original;
		original.show();
		original.window.resizeDocks(original.docks, {500, 200, 400, 200, 276}, Qt::Horizontal);
		settle(original.window);
		state = persist(original.proportions.StateToSave());
	}
	QList<QSize> expected;
	{
		Window first;
		first.window.resize(700, 900);
		first.restore(state);
		first.show();
		// No user edit: a clamped first session must preserve #2225's state.
		assert_true(persist(first.proportions.StateToSave()) == state);
		first.window.resize(1600, 900);
		settle(first.window);
		first.narrow_controls();
		expected = first.sizes();
		state = persist(first.proportions.StateToSave());
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
		state = persist(original.proportions.StateToSave());
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
		state = persist(first.proportions.StateToSave());
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
		state = persist(original.proportions.StateToSave());
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
		state = persist(first.proportions.StateToSave());
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
		state = persist(original.proportions.StateToSave());
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
		state = persist(first.proportions.StateToSave());
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
		state = persist(original.proportions.StateToSave());
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
		state = persist(first.proportions.StateToSave());
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
		state = persist(original.proportions.StateToSave());
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
		state = persist(first.proportions.StateToSave());
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
	return persist(original.proportions.StateToSave());
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
		const QByteArray saved = persist(first.proportions.StateToSave());
		Window restarted;
		restarted.restore(saved);
		restarted.show();
		print_message("floated dock after restart: floating=%d\n", restarted.docks[0]->isFloating());
		assert_true(restarted.docks[0]->isFloating());
	}
}

/* #5230: Reset Docks restores the default layout (startingDockLayout). When
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
		first.window.restoreState(first.startingLayout);
		first.window.resize(1200, 700);
		settle(first.window);
		print_message("dock 2 area after reset and resize: %d\n",
			      int(first.window.dockWidgetArea(first.docks[2])));
		assert_int_equal(first.window.dockWidgetArea(first.docks[2]), Qt::BottomDockWidgetArea);
		const QByteArray saved = persist(first.proportions.StateToSave());
		Window restarted;
		restarted.restore(saved);
		restarted.show();
		print_message("dock 2 area after restart: %d\n",
			      int(restarted.window.dockWidgetArea(restarted.docks[2])));
		assert_int_equal(restarted.window.dockWidgetArea(restarted.docks[2]), Qt::BottomDockWidgetArea);
	}
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
		w.window.restoreState(w.startingLayout);
		w.proportions.SetRestoredState(QByteArray());
		settle(w.window);
	}

	/* OBSBasic::saveAll writes what the helper selects. */
	QByteArray quit() { return persist(w.proportions.StateToSave()); }
};

struct DockLayout {
	Qt::DockWidgetArea area;
	bool floating;
	bool visible;
	QSize size;
};

static QList<DockLayout> capture(const Window &w)
{
	QList<DockLayout> result;
	for (auto *dock : w.docks) {
		result.append({w.window.dockWidgetArea(dock), dock->isFloating(), dock->isVisible(), dock->size()});
	}
	return result;
}

static void print_layout(const char *what, const QList<DockLayout> &layout)
{
	for (qsizetype i = 0; i < layout.size(); ++i) {
		print_message("%s dock %d: area=%d floating=%d visible=%d %dx%d\n", what, int(i), int(layout[i].area),
			      layout[i].floating, layout[i].visible, layout[i].size.width(), layout[i].size.height());
	}
}

/* Same placement and visibility; sizes within 2 px for docks on screen,
 * unless the case compares placement only. */
static void assert_layout(const QList<DockLayout> &expected, const QList<DockLayout> &actual, bool sizes = true)
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
static QList<DockLayout> restart(Wm wm, const QByteArray &saved)
{
	Session next(wm);
	next.start(saved);
	return capture(next.w);
}

/* A saved layout as a floating window manager shows it at full size: the
 * same reference window for every profile. */
static QList<DockLayout> arranged_at_full_size(const QByteArray &state)
{
	Session reference(Wm::floating);
	reference.start(state);
	return capture(reference.w);
}

/* A case gives the layout the user expects, the layout after a restart under
 * the same window manager, and what quit saved. Each profile must give what
 * the user expects, and both must save the same layout (W2). */
struct Outcome {
	QList<DockLayout> expected;
	QList<DockLayout> restarted;
	QByteArray saved;
};

static void run_both(Outcome (*run)(Wm), bool sizes = true)
{
	QList<DockLayout> saved[2];
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
	QList<DockLayout> in_session;
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
	const QList<DockLayout> expected = arranged_at_full_size(state);
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
	QList<DockLayout> expected;
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
	QList<DockLayout> expected;
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
	QList<DockLayout> expected;
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
		state = persist(original.proportions.StateToSave());
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

int main(int argc, char **argv)
{
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
	};
	return cmocka_run_group_tests(tests, nullptr, nullptr);
}
