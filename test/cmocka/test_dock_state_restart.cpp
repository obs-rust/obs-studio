/* #3740: a user can deliberately leave a dock at its minimum width.
 * Exercise the production dock-state selector, Qt's layout serialization and
 * the same config C API / base64 / safe-save / reopen path as OBSBasic. */
#include <QApplication>
#include <QDockWidget>
#include <QLabel>
#include <QMainWindow>
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

int main(int argc, char **argv)
{
	QApplication app(argc, argv);
	const CMUnitTest tests[] = {
		cmocka_unit_test(changed_layout_at_minimum_survives_restart),
		cmocka_unit_test(changed_layout_after_clamped_start_survives_restart),
		cmocka_unit_test(separator_click_does_not_save_a_clamped_layout),
	};
	return cmocka_run_group_tests(tests, nullptr, nullptr);
}
