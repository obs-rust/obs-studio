/* Undo entries from a properties view (shared/properties-view) when several
 * fields change within its 500 ms coalescing window
 * (obsproject/obs-studio#10628). The Filters dialog adds one undo entry per
 * update callback, with the old and new settings it is given
 * (FilterChangeUndoRedo). Setting Crop/Pad Left = 1 and then Top = 2 within
 * 500 ms gave Left's entry the new settings {left 1, top 2}, so Undo twice
 * and Redo once ended at Left = 1, Top = 2 instead of Left = 1, Top = 0.
 *
 * Builds the real OBSPropertiesView with two integer fields like Crop/Pad's
 * and records each callback; Qt's offscreen platform, no display, no
 * frontend build. */

#include <QApplication>
#include <QSpinBox>
#include <QTest>

#include <obs.hpp>

#include "properties-view.hpp"

#include <string>
#include <utility>
#include <vector>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

/* properties-view picks its help icon by theme. */
extern "C" bool obs_frontend_is_theme_dark(void)
{
	return false;
}

struct Entry {
	long long oldLeft, oldTop, newLeft, newTop;
};

static std::vector<Entry> entries;
static int dummy_object;

static obs_properties_t *crop_properties(void *)
{
	obs_properties_t *props = obs_properties_create();
	obs_properties_add_int(props, "left", "Left", -8192, 8192, 1);
	obs_properties_add_int(props, "top", "Top", -8192, 8192, 1);
	return props;
}

static void record_update(void *, obs_data_t *old_settings, obs_data_t *new_settings)
{
	entries.push_back({obs_data_get_int(old_settings, "left"), obs_data_get_int(old_settings, "top"),
			   obs_data_get_int(new_settings, "left"), obs_data_get_int(new_settings, "top")});
}

struct Fixture {
	OBSDataAutoRelease settings = obs_data_create();
	OBSPropertiesView *view = nullptr;
	QSpinBox *left = nullptr;
	QSpinBox *top = nullptr;
};

static int setup(void **state)
{
	entries.clear();
	auto *f = new Fixture;
	obs_data_set_int(f->settings, "left", 0);
	obs_data_set_int(f->settings, "top", 0);
	f->view = new OBSPropertiesView(f->settings.Get(), (void *)&dummy_object, crop_properties, record_update);
	f->view->show();
	/* The view builds its widgets from a queued call. */
	QTest::qWait(50);
	QList<QSpinBox *> spins = f->view->findChildren<QSpinBox *>();
	if (spins.size() != 2) {
		delete f->view;
		delete f;
		return -1;
	}
	f->left = spins[0];
	f->top = spins[1];
	*state = f;
	return 0;
}

static int teardown(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	delete f->view;
	delete f;
	entries.clear();
	return 0;
}

/* Past the 500 ms window: every pending entry has been added. */
static void settle()
{
	QTest::qWait(700);
}

static void assert_entry(size_t i, long long oldLeft, long long oldTop, long long newLeft, long long newTop)
{
	assert_true(i < entries.size());
	assert_int_equal(entries[i].oldLeft, oldLeft);
	assert_int_equal(entries[i].oldTop, oldTop);
	assert_int_equal(entries[i].newLeft, newLeft);
	assert_int_equal(entries[i].newTop, newTop);
}

/* The issue's steps: Left = 1, Top = 2 within 500 ms, then Undo, Undo, Redo
 * (an undo applies an entry's old settings, a redo its new ones). */
static void two_fields_within_window(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	f->left->setValue(1);
	QTest::qWait(100);
	f->top->setValue(2);
	settle();

	/* Undo twice applies entry 1's and then entry 0's old settings, Redo
	 * once entry 0's new settings: they must be Left = 1, Top = 0. */
	assert_int_equal(entries.size(), 2);
	assert_entry(0, 0, 0, 1, 0);
	assert_entry(1, 1, 0, 1, 2);
}

/* Left, Top, Left again within 500 ms: three entries, each with the state
 * right before and right after its own change. */
static void back_and_forth_within_window(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	f->left->setValue(1);
	QTest::qWait(100);
	f->top->setValue(2);
	QTest::qWait(100);
	f->left->setValue(5);
	settle();

	assert_int_equal(entries.size(), 3);
	assert_entry(0, 0, 0, 1, 0);
	assert_entry(1, 1, 0, 1, 2);
	assert_entry(2, 1, 2, 5, 2);
}

/* Control: changes to one field within 500 ms are one entry (typing "12"). */
static void one_field_coalesces(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	f->left->setValue(1);
	QTest::qWait(100);
	f->left->setValue(12);
	settle();

	assert_int_equal(entries.size(), 1);
	assert_entry(0, 0, 0, 12, 0);
}

/* Control: two fields more than 500 ms apart are two correct entries. */
static void two_fields_apart(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	f->left->setValue(1);
	settle();
	f->top->setValue(2);
	settle();

	assert_int_equal(entries.size(), 2);
	assert_entry(0, 0, 0, 1, 0);
	assert_entry(1, 1, 0, 1, 2);
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(two_fields_within_window, setup, teardown),
		cmocka_unit_test_setup_teardown(back_and_forth_within_window, setup, teardown),
		cmocka_unit_test_setup_teardown(one_field_coalesces, setup, teardown),
		cmocka_unit_test_setup_teardown(two_fields_apart, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
