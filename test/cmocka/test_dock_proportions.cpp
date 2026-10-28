/* Dock sizes after the main window is narrowed and widened again
 * (obs-rust/obs-studio#2225). OBS's default layout puts the Scenes, Sources,
 * Audio Mixer, Transitions and Controls docks in one row along the bottom.
 * Narrowing the window below the docks' minimum widths shrank each of them to
 * its minimum, and widening it again shared the width out equally, so a
 * layout the user had sized by hand came back with every dock the same width.
 *
 * Builds a QMainWindow with such a row of docks, plus the helper OBSBasic
 * installs (frontend/utility/dock-proportions.hpp), on Qt's offscreen
 * platform. */

#include <QApplication>
#include <QDockWidget>
#include <QLabel>
#include <QMainWindow>
#include <QTest>

#include "dock-proportions.hpp"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
extern "C" {
#include <cmocka.h>
}

static const int dock_min_width = 150;
static const QList<int> hand_sized = {720, 224, 256, 200, 200};

struct Fixture {
	QMainWindow *window = nullptr;
	DockProportions *proportions = nullptr;
	QList<QDockWidget *> docks;
};

static int setup(void **state)
{
	auto *f = new Fixture;
	f->window = new QMainWindow;
	f->window->setCentralWidget(new QLabel(QStringLiteral("preview")));
	f->proportions = new DockProportions(f->window);
	for (int i = 0; i < hand_sized.size(); i++) {
		auto *dock = new QDockWidget(QString::number(i));
		dock->setObjectName(QStringLiteral("dock%1").arg(i));
		dock->setWidget(new QLabel(QStringLiteral("contents")));
		dock->widget()->setMinimumWidth(dock_min_width);
		f->window->addDockWidget(Qt::BottomDockWidgetArea, dock);
		if (i) {
			f->window->splitDockWidget(f->docks.last(), dock, Qt::Horizontal);
		}
		f->docks.append(dock);
	}
	f->window->resize(1600, 900);
	f->window->show();
	QTest::qWait(50);
	f->window->resizeDocks(f->docks, hand_sized, Qt::Horizontal);
	QTest::qWait(50);
	*state = f;
	return 0;
}

static int teardown(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	delete f->window;
	delete f;
	return 0;
}

static QList<int> widths(const Fixture *f)
{
	QList<int> result;
	for (QDockWidget *dock : f->docks) {
		result.append(dock->width());
	}
	return result;
}

static void print_widths(const char *when, const QList<int> &w)
{
	printf("%s:", when);
	for (int width : w) {
		printf(" %d", width);
	}
	printf("\n");
}

/* Each dock's share of the row is within 2 percentage points of before. */
static void assert_same_shares(const QList<int> &before, const QList<int> &after)
{
	int total_before = 0, total_after = 0;
	for (int i = 0; i < before.size(); i++) {
		total_before += before[i];
		total_after += after[i];
	}
	for (int i = 0; i < before.size(); i++) {
		int share_before = before[i] * 1000 / total_before;
		int share_after = after[i] * 1000 / total_after;
		if (abs(share_before - share_after) > 20) {
			print_error("dock %d: %d.%d%% of the row before, %d.%d%% after\n", i, share_before / 10,
				    share_before % 10, share_after / 10, share_after % 10);
			fail();
		}
	}
}

static void narrow_then_wide_keeps_shares(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	const QList<int> before = widths(f);
	print_widths("before", before);

	f->window->resize(700, 900);
	QTest::qWait(50);
	print_widths("narrow", widths(f));
	/* The narrow window really clamps the docks to their minimum. */
	for (int width : widths(f)) {
		assert_true(width <= dock_min_width + 10);
	}

	f->window->resize(1600, 900);
	QTest::qWait(50);
	const QList<int> after = widths(f);
	print_widths("after", after);
	assert_same_shares(before, after);
}

static void narrow_in_steps_then_wide_keeps_shares(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	const QList<int> before = widths(f);

	/* Dragging the window edge: many small resizes. */
	for (int w = 1600; w >= 700; w -= 50) {
		f->window->resize(w, 900);
		QTest::qWait(10);
	}
	for (int w = 700; w <= 1600; w += 50) {
		f->window->resize(w, 900);
		QTest::qWait(10);
	}
	QTest::qWait(50);
	const QList<int> after = widths(f);
	print_widths("after steps", after);
	assert_same_shares(before, after);
}

static void dragged_separator_is_kept(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	/* The user re-sizes the docks by hand after the first layout. */
	const QList<int> dragged = {400, 400, 300, 200, 200};
	f->window->resizeDocks(f->docks, dragged, Qt::Horizontal);
	QTest::qWait(50);
	const QList<int> before = widths(f);

	f->window->resize(700, 900);
	QTest::qWait(50);
	f->window->resize(1600, 900);
	QTest::qWait(50);
	assert_same_shares(before, widths(f));
}

static void unclamped_resize_is_untouched(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	const QList<int> before = widths(f);

	/* Wide enough that no dock reaches its minimum: Qt keeps the shares. */
	f->window->resize(1400, 900);
	QTest::qWait(50);
	f->window->resize(1600, 900);
	QTest::qWait(50);
	assert_same_shares(before, widths(f));
}

/* OBS's default layout also has a column on the left: Scenes above Sources,
 * the full height of the window, beside the preview and the bottom row.
 * Narrowing the window shrank the column, and widening it again gave all of
 * the new width to the preview, so the column stayed narrow. */
struct ColumnFixture {
	QMainWindow *window = nullptr;
	QList<QDockWidget *> column;
};

static const int column_width = 280;

static int setup_column(void **state)
{
	auto *f = new ColumnFixture;
	f->window = new QMainWindow;
	f->window->setCentralWidget(new QLabel(QStringLiteral("preview")));
	/* As OBSBasic does with its side docks set to full height. */
	f->window->setCorner(Qt::TopLeftCorner, Qt::LeftDockWidgetArea);
	f->window->setCorner(Qt::BottomLeftCorner, Qt::LeftDockWidgetArea);
	new DockProportions(f->window);
	for (int i = 0; i < 2; i++) {
		auto *dock = new QDockWidget(QString::number(i));
		dock->setObjectName(QStringLiteral("side%1").arg(i));
		dock->setWidget(new QLabel(QStringLiteral("contents")));
		dock->widget()->setMinimumWidth(dock_min_width);
		f->window->addDockWidget(Qt::LeftDockWidgetArea, dock, Qt::Vertical);
		f->column.append(dock);
	}
	/* The bottom row beside it: its minimum widths are what force the
	 * column to shrink when the window narrows. */
	QDockWidget *previous = nullptr;
	for (int i = 0; i < 3; i++) {
		auto *dock = new QDockWidget(QString::number(i));
		dock->setObjectName(QStringLiteral("bottom%1").arg(i));
		dock->setWidget(new QLabel(QStringLiteral("contents")));
		dock->widget()->setMinimumWidth(dock_min_width);
		f->window->addDockWidget(Qt::BottomDockWidgetArea, dock);
		if (previous) {
			f->window->splitDockWidget(previous, dock, Qt::Horizontal);
		}
		previous = dock;
	}
	f->window->resize(1600, 900);
	f->window->show();
	QTest::qWait(50);
	f->window->resizeDocks({f->column.first()}, {column_width}, Qt::Horizontal);
	QTest::qWait(50);
	*state = f;
	return 0;
}

static int teardown_column(void **state)
{
	auto *f = static_cast<ColumnFixture *>(*state);
	delete f->window;
	delete f;
	return 0;
}

static void assert_column_width(const ColumnFixture *f, int expected)
{
	for (QDockWidget *dock : f->column) {
		if (abs(dock->width() - expected) > 4) {
			print_error("%s is %d px wide, expected %d\n", qPrintable(dock->objectName()), dock->width(),
				    expected);
			fail();
		}
	}
}

static void side_column_comes_back_after_narrow_then_wide(void **state)
{
	auto *f = static_cast<ColumnFixture *>(*state);
	assert_column_width(f, column_width);

	f->window->resize(700, 900);
	QTest::qWait(50);
	printf("narrow: window %d px, column %d px\n", f->window->width(), f->column.first()->width());
	/* The narrow window really shrinks the column. */
	assert_true(f->column.first()->width() < column_width - 30);

	f->window->resize(1600, 900);
	QTest::qWait(50);
	printf("after: column %d px\n", f->column.first()->width());
	assert_column_width(f, column_width);
}

static void side_column_comes_back_after_steps(void **state)
{
	auto *f = static_cast<ColumnFixture *>(*state);

	/* Dragging the window edge: many small resizes. */
	for (int w = 1600; w >= 700; w -= 50) {
		f->window->resize(w, 900);
		QTest::qWait(10);
	}
	for (int w = 700; w <= 1600; w += 50) {
		f->window->resize(w, 900);
		QTest::qWait(10);
	}
	QTest::qWait(50);
	printf("after steps: column %d px\n", f->column.first()->width());
	assert_column_width(f, column_width);
}

static void dragged_column_width_is_kept(void **state)
{
	auto *f = static_cast<ColumnFixture *>(*state);
	/* The user drags the column's edge after the first layout. */
	f->window->resizeDocks({f->column.first()}, {400}, Qt::Horizontal);
	QTest::qWait(50);
	const int dragged = f->column.first()->width();
	assert_true(dragged > column_width + 50);

	f->window->resize(700, 900);
	QTest::qWait(50);
	f->window->resize(1600, 900);
	QTest::qWait(50);
	assert_column_width(f, dragged);
}

/* The state the app writes to its ini must not be a clamped one: a session
 * that is narrow when it saves would otherwise come back cramped at full
 * width on the next start (obs-rust/obs-studio#2225). */
static QList<int> restored_widths(const QByteArray &state)
{
	QMainWindow window;
	window.setCentralWidget(new QLabel(QStringLiteral("preview")));
	QList<QDockWidget *> docks;
	for (int i = 0; i < hand_sized.size(); i++) {
		auto *dock = new QDockWidget(QString::number(i));
		dock->setObjectName(QStringLiteral("dock%1").arg(i));
		dock->setWidget(new QLabel(QStringLiteral("contents")));
		dock->widget()->setMinimumWidth(dock_min_width);
		window.addDockWidget(Qt::BottomDockWidgetArea, dock);
		if (i) {
			window.splitDockWidget(docks.last(), dock, Qt::Horizontal);
		}
		docks.append(dock);
	}
	window.resize(1600, 900);
	assert_true(window.restoreState(state));
	window.show();
	QTest::qWait(50);

	QList<int> result;
	for (QDockWidget *dock : docks) {
		result.append(dock->width());
	}
	return result;
}

static void clamped_layout_is_not_the_saved_state(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	const QList<int> before = widths(f);
	print_widths("before", before);

	f->window->resize(700, 900);
	QTest::qWait(50);
	print_widths("narrow", widths(f));
	for (int width : widths(f)) {
		assert_true(width <= dock_min_width + 10);
	}

	const QList<int> after = restored_widths(f->proportions->StateToSave());
	print_widths("saved while narrow, restored at 1600", after);
	assert_same_shares(before, after);
}

/* The other half of the same loss: the layout the ini already holds. A window
 * restored on a display too small for it comes up with every dock at its
 * minimum, and saving from that first session overwrote the layout the user
 * still had (obs-rust/obs-studio#2225). */
static void restored_layout_survives_a_narrow_first_session(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	const QList<int> before = widths(f);
	print_widths("hand sized", before);
	/* What the app wrote to its ini from a non-clamped session. */
	const QByteArray saved = f->proportions->StateToSave();

	/* The next start: that layout restored into a window narrower than the
	 * docks' minimum widths. */
	QMainWindow window;
	window.setCentralWidget(new QLabel(QStringLiteral("preview")));
	auto *proportions = new DockProportions(&window);
	proportions->SetRestoredState(saved);
	QList<QDockWidget *> docks;
	for (int i = 0; i < hand_sized.size(); i++) {
		auto *dock = new QDockWidget(QString::number(i));
		dock->setObjectName(QStringLiteral("dock%1").arg(i));
		dock->setWidget(new QLabel(QStringLiteral("contents")));
		dock->widget()->setMinimumWidth(dock_min_width);
		window.addDockWidget(Qt::BottomDockWidgetArea, dock);
		if (i) {
			window.splitDockWidget(docks.last(), dock, Qt::Horizontal);
		}
		docks.append(dock);
	}
	window.resize(700, 900);
	assert_true(window.restoreState(saved));
	window.show();
	QTest::qWait(50);

	QList<int> narrow;
	for (QDockWidget *dock : docks) {
		narrow.append(dock->width());
	}
	print_widths("restored narrow", narrow);
	/* The narrow window really clamps the docks to their minimum. */
	for (int width : narrow) {
		assert_true(width <= dock_min_width + 10);
	}

	const QList<int> after = restored_widths(proportions->StateToSave());
	print_widths("saved from the narrow start, restored at 1600", after);
	assert_same_shares(before, after);
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(narrow_then_wide_keeps_shares, setup, teardown),
		cmocka_unit_test_setup_teardown(narrow_in_steps_then_wide_keeps_shares, setup, teardown),
		cmocka_unit_test_setup_teardown(dragged_separator_is_kept, setup, teardown),
		cmocka_unit_test_setup_teardown(unclamped_resize_is_untouched, setup, teardown),
		cmocka_unit_test_setup_teardown(side_column_comes_back_after_narrow_then_wide, setup_column,
						teardown_column),
		cmocka_unit_test_setup_teardown(side_column_comes_back_after_steps, setup_column, teardown_column),
		cmocka_unit_test_setup_teardown(dragged_column_width_is_kept, setup_column, teardown_column),
		cmocka_unit_test_setup_teardown(clamped_layout_is_not_the_saved_state, setup, teardown),
		cmocka_unit_test_setup_teardown(restored_layout_survives_a_narrow_first_session, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
