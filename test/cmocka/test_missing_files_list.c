/* Regression test for obsproject/obs-studio#11074 (reported by kamalmostafa):
 * with --disable-missing-files-check, OBS leaks the list of missing files of
 * the scene collection it loads ("Number of memory leaks: 5" on exit with one
 * missing image file).
 *
 * OBSBasic::LoadData() makes the list, fills it while it loads the sources
 * and transitions, and hands it to the Missing Files dialog, which frees it.
 * The flag only skips the dialog, so nothing frees the list. With the check
 * disabled no list may be made, and loading must still work without one.
 *
 * The test drives the Qt-free helper LoadData() calls. The first case needs
 * no libobs core. The others load real libobs sources of a minimal registered
 * type that reports one missing file, with a filter that reports another; they
 * need no video or graphics. obs_startup() needs a display on Linux (run under
 * xvfb-run); those cases skip when it cannot start. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>

#include "missing-files-list.h"

#define INPUT_ID "missing_files_list_input"
#define FILTER_ID "missing_files_list_filter"

static bool obs_up;

static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static const char *type_name(void *type_data)
{
	(void)type_data;
	return "missing files test";
}

static void *data_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void data_destroy(void *data)
{
	(void)data;
}

static uint32_t type_size(void *data)
{
	(void)data;
	return 64;
}

static obs_missing_files_t *one_missing_file(const char *path, void *source)
{
	obs_missing_files_t *files = obs_missing_files_create();
	obs_missing_files_add_file(files, obs_missing_file_create(path, NULL, OBS_MISSING_FILE_SOURCE, source, NULL));
	return files;
}

static obs_missing_files_t *input_missing_files(void *data)
{
	return one_missing_file("/nonexistent/missing-files-list/input.png", data);
}

static obs_missing_files_t *filter_missing_files(void *data)
{
	return one_missing_file("/nonexistent/missing-files-list/filter.cube", data);
}

static void register_types(void)
{
	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = data_create,
		.destroy = data_destroy,
		.get_width = type_size,
		.get_height = type_size,
		.missing_files = input_missing_files,
	};
	struct obs_source_info filter = {
		.id = FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = data_create,
		.destroy = data_destroy,
		.missing_files = filter_missing_files,
	};
	obs_register_source(&input);
	obs_register_source(&filter);
}

static void no_task(void *param)
{
	(void)param;
}

/* obs_load_sources() drops its references once it has loaded the array; let
 * libobs's destruction thread finish with the loaded sources. */
static void flush_destroyed(void)
{
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
}

/* What a saved scene collection holds for one input with one filter. */
static obs_data_array_t *saved_sources(void)
{
	obs_source_t *input = obs_source_create(INPUT_ID, "input with a missing file", NULL, NULL);
	obs_source_t *filter = obs_source_create(FILTER_ID, "filter with a missing file", NULL, NULL);
	obs_data_array_t *array = obs_data_array_create();
	obs_data_t *saved;

	assert_non_null(input);
	assert_non_null(filter);
	obs_source_filter_add(input, filter);
	saved = obs_save_source(input);
	obs_data_array_push_back(array, saved);

	obs_data_release(saved);
	obs_source_filter_remove(input, filter);
	obs_source_release(filter);
	obs_source_remove(input);
	obs_source_release(input);
	flush_destroyed();
	return array;
}

/* With the check disabled nothing would free the list, so none is made. */
static void test_disabled_makes_no_list(void **state)
{
	(void)state;
	long before = bnum_allocs();
	obs_missing_files_t *files = missing_files_list_create(true);

	assert_int_equal(bnum_allocs(), before);
	assert_null(files);
}

/* Control: with the check enabled the list is made and filled from the
 * source and its filter. */
static void test_enabled_collects(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_data_array_t *saved = saved_sources();
	obs_missing_files_t *files = missing_files_list_create(false);

	assert_non_null(files);
	obs_load_sources(saved, missing_files_list_add_source, files);
	assert_int_equal(obs_missing_files_count(files), 2);

	obs_missing_files_destroy(files);
	obs_data_array_release(saved);
	flush_destroyed();
}

/* Loading with the check disabled, as LoadData() does: nothing is left
 * allocated for a dialog that is never shown. */
static void test_disabled_load_leaves_nothing(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_data_array_t *saved = saved_sources();
	long before = bnum_allocs();
	obs_missing_files_t *files = missing_files_list_create(true);

	obs_load_sources(saved, missing_files_list_add_source, files);

	flush_destroyed();

	/* LoadData() does not show the dialog, so this is everything that
	 * frees the list. */
	assert_int_equal(bnum_allocs(), before);
	assert_null(files);
	obs_data_array_release(saved);
}

/* Show Missing Files enumerates every source with the same callback. */
static void test_add_source_without_list(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *input = obs_source_create(INPUT_ID, "input without a list", NULL, NULL);
	long before = bnum_allocs();

	missing_files_list_add_source(NULL, input);
	assert_int_equal(bnum_allocs(), before);

	obs_source_remove(input);
	obs_source_release(input);
	flush_destroyed();
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up)
		register_types();
	else
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_disabled_makes_no_list),
		cmocka_unit_test(test_enabled_collects),
		cmocka_unit_test(test_disabled_load_leaves_nothing),
		cmocka_unit_test(test_add_source_without_list),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
