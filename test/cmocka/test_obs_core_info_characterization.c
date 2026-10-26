/* Characterization of the core-info exports in libobs/obs.c.h and the module
 * getters in libobs/obs-module.c.h: obs_initialized(), obs_get_version(),
 * obs_get_version_string(), obs_set_cmdline_args()/obs_get_cmdline_args(),
 * obs_set_locale()/obs_get_locale(), obs_find_data_file(),
 * obs_add_data_path()/obs_remove_data_path(), obs_get_audio_info() and
 * obs_get_audio_info2() before any audio reset, and the NULL-module paths of
 * obs_get_module_author(), _description(), _lib(), _binary_path(),
 * _data_path(), obs_module_get_config_path(), obs_find_module_file() plus
 * obs_get_module_allow_disable(). It pins the CURRENT behaviour before the
 * Rust port, through the public API only, so the same test runs on the hybrid
 * libobs and libobs-c-reference. Cases marked "characterized, not endorsed"
 * are odd but real.
 *
 * obs_module_get_locale_string() and obs_module_get_locale_text() are not
 * covered: they assert(mod), so a NULL module aborts debug builds, and a real
 * module needs build plumbing (a loadable plugin) this test does not add.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. It skips only when no display is set at all; with one set, a failed
 * obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <obs-config.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/platform.h>

/* The deprecated calls are still exported; their behavior is pinned before
 * the port like every other export. */
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

#define PROBE_FILE "obs_core_info_characterization_probe.txt"

static bool obs_up;
static bool initialized_before_startup;

static int group_setup(void **state)
{
	(void)state;
	initialized_before_startup = obs_initialized();
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

#define SKIP_WITHOUT_OBS()          \
	do {                        \
		if (!obs_up)        \
			skip();     \
	} while (0)

static void test_initialized(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();
	assert_false(initialized_before_startup);
	assert_true(obs_initialized());
}

static void test_version(void **state)
{
	(void)state;
	uint32_t ver = obs_get_version();
	assert_int_equal(ver, LIBOBS_API_VER);
	assert_int_equal(ver >> 24, LIBOBS_API_MAJOR_VER);
	assert_int_equal((ver >> 16) & 0xff, LIBOBS_API_MINOR_VER);
	assert_int_equal(ver & 0xffff, LIBOBS_API_PATCH_VER);

	const char *str = obs_get_version_string();
	assert_non_null(str);
	assert_true(str[0] != '\0');
	/* Static string: the same pointer every call. */
	assert_ptr_equal(str, obs_get_version_string());
}

static void test_cmdline_args(void **state)
{
	(void)state;
	const char *argv[] = {"obs", "--verbose", ""};
	obs_set_cmdline_args(3, argv);

	struct obs_cmdline_args args = obs_get_cmdline_args();
	assert_int_equal(args.argc, 3);
	assert_non_null(args.argv);
	assert_string_equal(args.argv[0], "obs");
	assert_string_equal(args.argv[1], "--verbose");
	assert_string_equal(args.argv[2], "");
	assert_null(args.argv[3]);
	/* Copied, not borrowed. */
	assert_ptr_not_equal(args.argv[0], argv[0]);

	/* Once argc is non-zero a second call is ignored. */
	const char *other[] = {"other"};
	obs_set_cmdline_args(1, other);
	struct obs_cmdline_args again = obs_get_cmdline_args();
	assert_int_equal(again.argc, 3);
	assert_ptr_equal(again.argv, args.argv);
	assert_string_equal(again.argv[0], "obs");
}

static void test_locale(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();
	assert_string_equal(obs_get_locale(), "en-US");

	char buf[] = "de-DE";
	obs_set_locale(buf);
	const char *loc = obs_get_locale();
	assert_string_equal(loc, "de-DE");
	/* Duplicated: the caller's buffer is not kept. */
	assert_ptr_not_equal(loc, buf);
	buf[0] = 'x';
	assert_string_equal(obs_get_locale(), "de-DE");

	obs_set_locale("en-US");
	assert_string_equal(obs_get_locale(), "en-US");
}

static void test_find_data_file(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	assert_null(obs_find_data_file("obs_core_info_characterization_missing.txt"));

	assert_true(os_quick_write_utf8_file(PROBE_FILE, "x", 1, false));

	assert_false(obs_remove_data_path("./"));
	obs_add_data_path("./");
	char *found = obs_find_data_file(PROBE_FILE);
	assert_non_null(found);
	assert_string_equal(found, "./" PROBE_FILE);
	bfree(found);
	assert_true(obs_remove_data_path("./"));
	assert_false(obs_remove_data_path("./"));
	assert_null(obs_find_data_file(PROBE_FILE));

	/* characterized, not endorsed: the data path and the file name are
	 * concatenated with no separator, so a path without a trailing slash
	 * acts as a file-name prefix. */
	obs_add_data_path("./obs_core_info_");
	found = obs_find_data_file("characterization_probe.txt");
	assert_non_null(found);
	assert_string_equal(found, "./" PROBE_FILE);
	bfree(found);
	assert_true(obs_remove_data_path("./obs_core_info_"));

	os_unlink(PROBE_FILE);
}

static void test_audio_info_without_reset(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	struct obs_audio_info oai = {12345, SPEAKERS_UNKNOWN};
	assert_false(obs_get_audio_info(&oai));
	assert_int_equal(oai.samples_per_sec, 12345);
	assert_false(obs_get_audio_info(NULL));

	struct obs_audio_info2 oai2;
	memset(&oai2, 0, sizeof(oai2));
	oai2.samples_per_sec = 777;
	assert_false(obs_get_audio_info2(&oai2));
	assert_int_equal(oai2.samples_per_sec, 777);
	assert_false(obs_get_audio_info2(NULL));
}

static void test_module_getters_null(void **state)
{
	(void)state;
	assert_null(obs_get_module_author(NULL));
	assert_null(obs_get_module_description(NULL));
	assert_null(obs_get_module_lib(NULL));
	assert_null(obs_get_module_binary_path(NULL));
	assert_null(obs_get_module_data_path(NULL));
	assert_null(obs_find_module_file(NULL, "x"));
	assert_null(obs_module_get_config_path(NULL, "x"));
}

static void test_module_allow_disable(void **state)
{
	(void)state;
	/* A name not in the build's core module list may be disabled. */
	assert_true(obs_get_module_allow_disable("obs-core-info-characterization-not-a-module"));
	assert_true(obs_get_module_allow_disable(""));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_initialized),         cmocka_unit_test(test_version),
		cmocka_unit_test(test_cmdline_args),        cmocka_unit_test(test_locale),
		cmocka_unit_test(test_find_data_file),      cmocka_unit_test(test_audio_info_without_reset),
		cmocka_unit_test(test_module_getters_null), cmocka_unit_test(test_module_allow_disable),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
