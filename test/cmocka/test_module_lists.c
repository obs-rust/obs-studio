/* Characterization of the module list calls in libobs/obs-module.c.h:
 * obs_log_loaded_modules(), obs_module_add_source(), _add_output(),
 * _add_encoder(), _add_service(), obs_find_module_file(),
 * obs_module_get_config_path(), obs_add_module_path(), obs_add_safe_module()
 * and obs_add_disabled_module(). It pins the CURRENT behaviour before they
 * are extracted and ported, through the public API only, so the same test
 * runs on the hybrid libobs and libobs-c-reference, on every host.
 *
 * test_core_module_load (Linux only) already pins the search paths, the
 * file and config path rules on opened modules and how the safe and
 * disabled lists match names during a load. This test adds what nothing
 * pins: what obs_log_loaded_modules() logs, the calls with no core, and the
 * calls on a module made with obs_create_disabled_module() on every host.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * Not called: obs_log_loaded_modules() with no core. It reads the core
 * without checking it, so it crashes (characterized, not endorsed).
 *
 * obs_startup() needs a display on Linux, so the core group runs under
 * xvfb-run there. It skips only when no display is set at all; with one set,
 * a failed obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/platform.h>

/* get_module_extension() of each host: obs_create_disabled_module() names a
 * module by its file name less that many characters. */
#if defined(_WIN32)
#define MODULE_EXT ".dll"
#elif defined(__APPLE__)
#define MODULE_EXT ""
#else
#define MODULE_EXT ".so"
#endif

#define STUB_NAME "module_lists_stub"
#define CONFIG_ROOT "module-lists-config"
#define DATA_DIR "module_lists_data"
#define DATA_FILE "present.txt"

/* ------------------------------------------------------------ log capture */

#define MAX_LINES 16

struct log_lines {
	int count;
	int level[MAX_LINES];
	char text[MAX_LINES][256];
};

static void record_line(int level, const char *format, va_list args, void *param)
{
	struct log_lines *lines = param;
	if (lines->count >= MAX_LINES) {
		lines->count++;
		return;
	}
	lines->level[lines->count] = level;
	vsnprintf(lines->text[lines->count], sizeof(lines->text[0]), format, args);
	lines->count++;
}

/* The lines obs_log_loaded_modules() logs, through a handler of our own. */
static void log_loaded_modules(struct log_lines *lines)
{
	log_handler_t prev = NULL;
	void *prev_param = NULL;
	base_get_log_handler(&prev, &prev_param);

	memset(lines, 0, sizeof(*lines));
	base_set_log_handler(record_line, lines);
	obs_log_loaded_modules();
	base_set_log_handler(prev, prev_param);
}

/* ---------------------------------------------------------------- no core */

/* With no core, the calls that check it do nothing and allocate nothing, and
 * the ones that check only the module return NULL for no module. */
static void no_core_calls(void **state)
{
	(void)state;
	assert_false(obs_initialized());

	long allocs = bnum_allocs();
	obs_add_module_path("bin", "data");
	obs_add_safe_module("safe");
	obs_add_disabled_module("disabled");
	obs_module_add_source(NULL, "id");
	obs_module_add_output(NULL, "id");
	obs_module_add_encoder(NULL, "id");
	obs_module_add_service(NULL, "id");
	assert_null(obs_find_module_file(NULL, "file"));
	assert_null(obs_module_get_config_path(NULL, "file"));
	assert_int_equal(bnum_allocs(), allocs);
}

/* ------------------------------------------------------------------ core */

static bool obs_up;

static int core_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", CONFIG_ROOT, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
	}
	return 0;
}

static int core_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

#define SKIP_WITHOUT_OBS()      \
	do {                    \
		if (!obs_up)    \
			skip(); \
	} while (0)

static obs_module_t *disabled_module(const char *file, const char *data_path)
{
	obs_module_t *mod = NULL;
	assert_true(obs_create_disabled_module(&mod, file, data_path, OBS_MODULE_DISABLED));
	assert_non_null(mod);
	return mod;
}

/* No module loaded: only the heading, at LOG_INFO. A disabled module is not
 * a loaded one, so it adds no line. */
static void log_no_loaded_modules(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	struct log_lines lines;
	log_loaded_modules(&lines);
	assert_int_equal(lines.count, 1);
	assert_int_equal(lines.level[0], LOG_INFO);
	assert_string_equal(lines.text[0], "  Loaded Modules:");

	disabled_module("bin/" STUB_NAME "_logged" MODULE_EXT, "nodata");
	log_loaded_modules(&lines);
	assert_int_equal(lines.count, 1);
	assert_string_equal(lines.text[0], "  Loaded Modules:");
}

/* obs_module_add_*() with no module: nothing is allocated, and no module
 * names the id. */
static void add_types_no_module(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	long allocs = bnum_allocs();
	obs_module_add_source(NULL, "module_lists_null");
	obs_module_add_output(NULL, "module_lists_null");
	obs_module_add_encoder(NULL, "module_lists_null");
	obs_module_add_service(NULL, "module_lists_null");
	obs_module_add_source(NULL, NULL);
	assert_int_equal(bnum_allocs(), allocs);

	assert_null(obs_source_get_module("module_lists_null"));
	assert_null(obs_output_get_module("module_lists_null"));
	assert_null(obs_encoder_get_module("module_lists_null"));
	assert_null(obs_service_get_module("module_lists_null"));
}

/* obs_module_add_*() with a module: each list gets its own copy of the id,
 * and only that kind's lookup names the module. */
static void add_types_to_module(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	obs_module_t *mod = disabled_module("bin/" STUB_NAME "_types" MODULE_EXT, "nodata");

	char source[] = "module_lists_source";
	char output[] = "module_lists_output";
	char encoder[] = "module_lists_encoder";
	char service[] = "module_lists_service";
	long allocs = bnum_allocs();
	obs_module_add_source(mod, source);
	obs_module_add_output(mod, output);
	obs_module_add_encoder(mod, encoder);
	obs_module_add_service(mod, service);
	/* Each call copies its id and grows an empty list once. */
	assert_int_equal(bnum_allocs() - allocs, 8);

	/* Copies: the caller's strings can change. */
	source[0] = output[0] = encoder[0] = service[0] = 'X';

	assert_ptr_equal(obs_source_get_module("module_lists_source"), mod);
	assert_int_equal(obs_source_load_state("module_lists_source"), OBS_MODULE_DISABLED);
	assert_ptr_equal(obs_output_get_module("module_lists_output"), mod);
	assert_ptr_equal(obs_encoder_get_module("module_lists_encoder"), mod);
	assert_ptr_equal(obs_service_get_module("module_lists_service"), mod);

	assert_null(obs_source_get_module("Xodule_lists_source"));
	assert_null(obs_source_get_module("module_lists_output"));
	assert_null(obs_output_get_module("module_lists_source"));
	assert_null(obs_encoder_get_module("module_lists_service"));
	assert_null(obs_service_get_module("module_lists_encoder"));

	/* The same id again: a second entry, no new list. */
	allocs = bnum_allocs();
	obs_module_add_source(mod, "module_lists_source");
	assert_int_equal(bnum_allocs() - allocs, 1);
	assert_ptr_equal(obs_source_get_module("module_lists_source"), mod);
}

/* obs_find_module_file(): the module's data path, '/', the file, when the
 * file exists; NULL when it doesn't, or with no module. */
static void find_module_file(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	assert_true(os_mkdir(DATA_DIR) != MKDIR_ERROR);
	FILE *file = fopen(DATA_DIR "/" DATA_FILE, "w");
	assert_non_null(file);
	fputs("x", file);
	fclose(file);

	obs_module_t *mod = disabled_module("bin/" STUB_NAME "_files" MODULE_EXT, DATA_DIR);

	char *found = obs_find_module_file(mod, DATA_FILE);
	char *missing = obs_find_module_file(mod, "missing.txt");
	char *none = obs_find_module_file(NULL, DATA_FILE);

	os_unlink(DATA_DIR "/" DATA_FILE);
	os_rmdir(DATA_DIR);

	assert_non_null(found);
	assert_string_equal(found, DATA_DIR "/" DATA_FILE);
	assert_null(missing);
	assert_null(none);
	bfree(found);
}

/* obs_module_get_config_path(): the core's module config path, '/', the
 * module's name (its file name less the host's module extension), '/', the
 * file. NULL only with no module. */
static void module_config_path(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	obs_module_t *mod = disabled_module("bin/" STUB_NAME MODULE_EXT, "nodata");

	char *path = obs_module_get_config_path(mod, "settings.json");
	assert_non_null(path);
	assert_string_equal(path, CONFIG_ROOT "/" STUB_NAME "/settings.json");
	bfree(path);

	/* No file: the module's config directory and a trailing '/'
	 * (characterized, not endorsed). */
	path = obs_module_get_config_path(mod, NULL);
	assert_non_null(path);
	assert_string_equal(path, CONFIG_ROOT "/" STUB_NAME "/");
	bfree(path);

	assert_null(obs_module_get_config_path(NULL, "settings.json"));
}

/* obs_add_module_path(): nothing without both paths; with both, a copy of
 * each. obs_add_safe_module() and obs_add_disabled_module(): nothing without
 * a name; with one, a copy. Empty strings are copied like any other. */
static void add_list_entries(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	long allocs = bnum_allocs();
	obs_add_module_path(NULL, "data");
	obs_add_module_path("bin", NULL);
	obs_add_module_path(NULL, NULL);
	obs_add_safe_module(NULL);
	obs_add_disabled_module(NULL);
	assert_int_equal(bnum_allocs(), allocs);

	/* Each first entry copies its strings and grows the empty list. */
	allocs = bnum_allocs();
	obs_add_module_path("module-lists-bin", "module-lists-data");
	assert_int_equal(bnum_allocs() - allocs, 3);
	allocs = bnum_allocs();
	obs_add_safe_module("module_lists_safe");
	assert_int_equal(bnum_allocs() - allocs, 2);
	allocs = bnum_allocs();
	obs_add_disabled_module("module_lists_disabled");
	assert_int_equal(bnum_allocs() - allocs, 2);

	/* Later entries: one copy per string. */
	allocs = bnum_allocs();
	obs_add_module_path("", "");
	obs_add_safe_module("");
	obs_add_disabled_module("");
	assert_int_equal(bnum_allocs() - allocs, 4);

	/* Whether a module may be disabled is not the disabled list: it is
	 * whether the name is a core module. */
	assert_true(obs_get_module_allow_disable("module_lists_disabled"));
	assert_true(obs_get_module_allow_disable("module_lists_not_listed"));
}

/* A NULL id is pushed as a NULL entry: the list grows, nothing is copied
 * (characterized, not endorsed). The type lookups strcmp() every entry, so
 * after this a lookup that reaches this module would crash; it runs last,
 * and nothing looks up a type after it. */
static void add_null_id(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	obs_module_t *mod = disabled_module("bin/" STUB_NAME "_null" MODULE_EXT, "nodata");
	long allocs = bnum_allocs();
	obs_module_add_source(mod, NULL);
	obs_module_add_output(mod, NULL);
	obs_module_add_encoder(mod, NULL);
	obs_module_add_service(mod, NULL);
	assert_int_equal(bnum_allocs() - allocs, 4);
}

int main(void)
{
	const struct CMUnitTest no_core_tests[] = {
		cmocka_unit_test(no_core_calls),
	};
	const struct CMUnitTest core_tests[] = {
		cmocka_unit_test(log_no_loaded_modules), cmocka_unit_test(add_types_no_module),
		cmocka_unit_test(add_types_to_module),   cmocka_unit_test(find_module_file),
		cmocka_unit_test(module_config_path),    cmocka_unit_test(add_list_entries),
		cmocka_unit_test(add_null_id),
	};

	int failed = cmocka_run_group_tests_name("no core", no_core_tests, NULL, NULL);
	failed += cmocka_run_group_tests_name("core", core_tests, core_setup, core_teardown);
	return failed;
}
