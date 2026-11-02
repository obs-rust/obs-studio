#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <limits.h>
#include <stdio.h>

#include "obs-windows-system-info.h"

/*
 * Characterization of the system info log rules of libobs/obs-windows.c,
 * moved to libobs/obs-windows-system-info.c before their Rust port:
 * - obs_windows_system_info_version(): log_system_info()'s win_ver;
 * - obs_windows_system_info_mb(): log_available_memory()'s sizes;
 * - obs_windows_system_info_gaming_logged(), _game_mode_retry(),
 *   _gaming_found(), _gaming_value_line(), _gaming_fallback() and
 *   _gaming_fallback_line(): log_gaming_features()'s gate, its second Game
 *   Mode read, and its five lines, put together here as obs-windows.c does
 *   (the value only when the read found it, the build only for a feature
 *   with a fallback);
 * - obs_windows_system_info_product_state() and _product_type():
 *   log_security_products_by_type()'s labels.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions take no Windows type and are hidden in libobs, so this test
 * compiles the C source into itself, calls only them, and runs on every OS.
 * test_win_wsc_product_refs (Windows only) runs the security product log
 * itself.
 */

#define ERROR_FILE_NOT_FOUND 2
#define ERROR_ACCESS_DENIED 5

static const char *const unset = "unset";

/* One feature's line, put together as log_gaming_features() does, or NULL
 * when none is logged; label and text are left alone then. */
static const char *line_of(int feature, int32_t status, uint32_t value, uint32_t win_build)
{
	static char line[128];
	const char *label = unset;
	const char *text = unset;
	bool logged = false;

	if (obs_windows_system_info_gaming_found(status))
		logged = obs_windows_system_info_gaming_value_line(feature, value, &label, &text);
	else if (obs_windows_system_info_gaming_fallback(feature))
		logged = obs_windows_system_info_gaming_fallback_line(feature, win_build, &label, &text);

	if (!logged) {
		assert_ptr_equal(label, unset);
		assert_ptr_equal(text, unset);
		return NULL;
	}

	snprintf(line, sizeof(line), "\t%s: %s", label, text);
	return line;
}

static void header_values(void **state)
{
	(void)state;

	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_ERROR_SUCCESS, 0);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_STATE_ON, 0);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_STATE_OFF, 1);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_STATE_SNOOZED, 2);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_STATE_EXPIRED, 3);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_PROVIDER_FIREWALL, 1);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_PROVIDER_ANTIVIRUS, 4);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_PROVIDER_ANTISPYWARE, 8);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_GAME_BAR, 0);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_ALLOWED, 1);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR, 2);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_BG_RECORDING, 3);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, 4);
	assert_int_equal(OBS_WINDOWS_SYSTEM_INFO_GAMING_FEATURES, 5);
}

static void version_packing(void **state)
{
	(void)state;

	assert_int_equal(obs_windows_system_info_version(6, 1), 0x601);
	assert_int_equal(obs_windows_system_info_version(6, 3), 0x603);
	assert_int_equal(obs_windows_system_info_version(10, 0), 0xA00);
	assert_int_equal(obs_windows_system_info_version(0, 0), 0);

	/* characterized, not endorsed: a minor of 256 or more runs into the
   * major (Windows never reports one) */
	assert_int_equal(obs_windows_system_info_version(10, 0x100), 0xB00);
	assert_int_equal(obs_windows_system_info_version(6, 0x1FF), 0x7FF);

	/* the major's high bits shift out of 32 bits */
	assert_int_equal(obs_windows_system_info_version(0x1000000, 1), 1);
	assert_int_equal(obs_windows_system_info_version(UINT32_MAX, 0), 0xFFFFFF00u);
}

static void gaming_gate(void **state)
{
	(void)state;

	/* Windows 11 reports 10.0 too */
	assert_true(obs_windows_system_info_gaming_logged(0xA00));
	assert_true(obs_windows_system_info_gaming_logged(0xA01));
	assert_true(obs_windows_system_info_gaming_logged(UINT32_MAX));
	assert_false(obs_windows_system_info_gaming_logged(0x9FF));
	assert_false(obs_windows_system_info_gaming_logged(0x603));
	assert_false(obs_windows_system_info_gaming_logged(0x601));
	assert_false(obs_windows_system_info_gaming_logged(0));
}

static void game_mode_retry(void **state)
{
	(void)state;

	assert_false(obs_windows_system_info_game_mode_retry(OBS_WINDOWS_SYSTEM_INFO_ERROR_SUCCESS));
	assert_true(obs_windows_system_info_game_mode_retry(ERROR_FILE_NOT_FOUND));
	assert_true(obs_windows_system_info_game_mode_retry(ERROR_ACCESS_DENIED));
	assert_true(obs_windows_system_info_game_mode_retry(1));
	assert_true(obs_windows_system_info_game_mode_retry(-1));
	assert_true(obs_windows_system_info_game_mode_retry(INT32_MIN));
	assert_true(obs_windows_system_info_game_mode_retry(INT32_MAX));
}

static void gaming_split(void **state)
{
	(void)state;
	const char *label = unset;
	const char *text = unset;

	/* a read found its value only with ERROR_SUCCESS */
	assert_true(obs_windows_system_info_gaming_found(OBS_WINDOWS_SYSTEM_INFO_ERROR_SUCCESS));
	assert_false(obs_windows_system_info_gaming_found(ERROR_FILE_NOT_FOUND));
	assert_false(obs_windows_system_info_gaming_found(ERROR_ACCESS_DENIED));
	assert_false(obs_windows_system_info_gaming_found(1));
	assert_false(obs_windows_system_info_gaming_found(-1));
	assert_false(obs_windows_system_info_gaming_found(INT32_MIN));
	assert_false(obs_windows_system_info_gaming_found(INT32_MAX));

	/* only Game Mode has a line without its value, so only for it is the
   * build read */
	assert_true(obs_windows_system_info_gaming_fallback(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE));
	for (int feature = OBS_WINDOWS_SYSTEM_INFO_GAME_BAR; feature <= OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_BG_RECORDING;
	     feature++) {
		assert_false(obs_windows_system_info_gaming_fallback(feature));
		/* and asked anyway, the others have no fallback line at any
     * build */
		assert_false(obs_windows_system_info_gaming_fallback_line(feature, 19042, &label, &text));
		assert_false(obs_windows_system_info_gaming_fallback_line(feature, UINT32_MAX, &label, &text));
	}
	assert_ptr_equal(label, unset);
	assert_ptr_equal(text, unset);

	/* every feature has a line for a found value */
	for (int feature = OBS_WINDOWS_SYSTEM_INFO_GAME_BAR; feature < OBS_WINDOWS_SYSTEM_INFO_GAMING_FEATURES;
	     feature++) {
		label = unset;
		text = unset;
		assert_true(obs_windows_system_info_gaming_value_line(feature, 0, &label, &text));
		assert_ptr_not_equal(label, unset);
		assert_ptr_not_equal(text, unset);
	}
}

static void gaming_lines(void **state)
{
	(void)state;
	const int32_t ok = OBS_WINDOWS_SYSTEM_INFO_ERROR_SUCCESS;

	/* the same bytes as the five formats it replaces */
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_BAR, ok, 1, 0), "\tGame Bar: On");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_BAR, ok, 0, 0), "\tGame Bar: Off");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_ALLOWED, ok, 1, 0), "\tGame DVR Allowed: Yes");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_ALLOWED, ok, 0, 0), "\tGame DVR Allowed: No");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR, ok, 1, 0), "\tGame DVR: On");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR, ok, 0, 0), "\tGame DVR: Off");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_BG_RECORDING, ok, 1, 0),
			    "\tGame DVR Background Recording: On");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_BG_RECORDING, ok, 0, 0),
			    "\tGame DVR Background Recording: Off");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ok, 1, 0), "\tGame Mode: On");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ok, 0, 0), "\tGame Mode: Off");

	/* characterized, not endorsed: any non-zero value is On (or Yes) */
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_BAR, ok, 2, 0), "\tGame Bar: On");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_BAR, ok, UINT32_MAX, 0), "\tGame Bar: On");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_ALLOWED, ok, UINT32_MAX, 0),
			    "\tGame DVR Allowed: Yes");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR, ok, 0x80000000u, 0), "\tGame DVR: On");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_BG_RECORDING, ok, 0x100, 0),
			    "\tGame DVR Background Recording: On");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ok, UINT32_MAX, 0), "\tGame Mode: On");

	/* a found value ignores the build */
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ok, 0, 19042), "\tGame Mode: Off");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_BAR, ok, 0, UINT32_MAX), "\tGame Bar: Off");
}

static void failed_reads(void **state)
{
	(void)state;
	const int32_t statuses[] = {ERROR_FILE_NOT_FOUND, ERROR_ACCESS_DENIED, 1, -1, INT32_MIN, INT32_MAX};

	/* the first four features log nothing without their value, at any
   * build and whatever the value */
	for (size_t i = 0; i < sizeof(statuses) / sizeof(statuses[0]); i++) {
		for (int feature = OBS_WINDOWS_SYSTEM_INFO_GAME_BAR;
		     feature <= OBS_WINDOWS_SYSTEM_INFO_GAME_DVR_BG_RECORDING; feature++) {
			assert_null(line_of(feature, statuses[i], 0, 0));
			assert_null(line_of(feature, statuses[i], 1, 19042));
			assert_null(line_of(feature, statuses[i], UINT32_MAX, UINT32_MAX));
		}
	}
}

static void game_mode_missing(void **state)
{
	(void)state;

	/* Game Mode is on by default from build 19042, where its key is often
   * missing */
	assert_null(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ERROR_FILE_NOT_FOUND, 0, 19041));
	assert_null(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ERROR_FILE_NOT_FOUND, 1, 0));
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ERROR_FILE_NOT_FOUND, 0, 19042),
			    "\tGame Mode: Probably On (no reg key set)");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ERROR_FILE_NOT_FOUND, 1, 22631),
			    "\tGame Mode: Probably On (no reg key set)");
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, -1, 0, UINT32_MAX),
			    "\tGame Mode: Probably On (no reg key set)");
	/* any failure counts as a missing key */
	assert_string_equal(line_of(OBS_WINDOWS_SYSTEM_INFO_GAME_MODE, ERROR_ACCESS_DENIED, 0, 19042),
			    "\tGame Mode: Probably On (no reg key set)");
}

static void unknown_features(void **state)
{
	(void)state;
	const int features[] = {-1, OBS_WINDOWS_SYSTEM_INFO_GAMING_FEATURES, 6, INT_MAX, INT_MIN};

	for (size_t i = 0; i < sizeof(features) / sizeof(features[0]); i++) {
		const char *label = unset;
		const char *text = unset;

		assert_null(line_of(features[i], OBS_WINDOWS_SYSTEM_INFO_ERROR_SUCCESS, 1, 19042));
		assert_null(line_of(features[i], ERROR_FILE_NOT_FOUND, 1, 19042));
		assert_false(obs_windows_system_info_gaming_value_line(features[i], 1, &label, &text));
		assert_false(obs_windows_system_info_gaming_fallback(features[i]));
		assert_false(obs_windows_system_info_gaming_fallback_line(features[i], 19042, &label, &text));
		assert_ptr_equal(label, unset);
		assert_ptr_equal(text, unset);
	}
}

static void product_labels(void **state)
{
	(void)state;

	assert_string_equal(obs_windows_system_info_product_state(OBS_WINDOWS_SYSTEM_INFO_STATE_ON), "enabled");
	assert_string_equal(obs_windows_system_info_product_state(OBS_WINDOWS_SYSTEM_INFO_STATE_OFF), "disabled");
	assert_string_equal(obs_windows_system_info_product_state(OBS_WINDOWS_SYSTEM_INFO_STATE_SNOOZED),
			    "temporarily disabled");
	assert_string_equal(obs_windows_system_info_product_state(OBS_WINDOWS_SYSTEM_INFO_STATE_EXPIRED), "expired");
	assert_string_equal(obs_windows_system_info_product_state(4), "unknown");
	assert_string_equal(obs_windows_system_info_product_state(-1), "unknown");
	assert_string_equal(obs_windows_system_info_product_state(INT_MAX), "unknown");
	assert_string_equal(obs_windows_system_info_product_state(INT_MIN), "unknown");

	assert_string_equal(obs_windows_system_info_product_type(OBS_WINDOWS_SYSTEM_INFO_PROVIDER_ANTIVIRUS), "AV");
	assert_string_equal(obs_windows_system_info_product_type(OBS_WINDOWS_SYSTEM_INFO_PROVIDER_FIREWALL), "FW");
	assert_string_equal(obs_windows_system_info_product_type(OBS_WINDOWS_SYSTEM_INFO_PROVIDER_ANTISPYWARE), "ASW");
	/* WSC_SECURITY_PROVIDER_AUTOUPDATE_SETTINGS and the others the log
   * never asks for */
	assert_string_equal(obs_windows_system_info_product_type(2), "unknown");
	assert_string_equal(obs_windows_system_info_product_type(0), "unknown");
	assert_string_equal(obs_windows_system_info_product_type(3), "unknown");
	assert_string_equal(obs_windows_system_info_product_type(16), "unknown");
	assert_string_equal(obs_windows_system_info_product_type(-1), "unknown");
	assert_string_equal(obs_windows_system_info_product_type(INT_MIN), "unknown");
}

static void memory_sizes(void **state)
{
	(void)state;

	assert_int_equal(obs_windows_system_info_mb(0), 0);
	assert_int_equal(obs_windows_system_info_mb(1), 0);
	assert_int_equal(obs_windows_system_info_mb(1048575), 0);
	assert_int_equal(obs_windows_system_info_mb(1048576), 1);
	assert_int_equal(obs_windows_system_info_mb(1048577), 1);
	assert_int_equal(obs_windows_system_info_mb(17179869184ull), 16384);

	/* characterized, not endorsed: the size is cut to 32 bits, as the
   * DWORD the log prints (4 PiB and more; unreachable in practice) */
	assert_int_equal(obs_windows_system_info_mb(0xFFFFFFFFull * 1048576), UINT32_MAX);
	assert_int_equal(obs_windows_system_info_mb(1ull << 52), 0);
	assert_int_equal(obs_windows_system_info_mb((1ull << 52) + 1048576), 1);
	assert_int_equal(obs_windows_system_info_mb(UINT64_MAX), UINT32_MAX);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(header_values),    cmocka_unit_test(version_packing),
		cmocka_unit_test(gaming_gate),      cmocka_unit_test(game_mode_retry),
		cmocka_unit_test(gaming_split),     cmocka_unit_test(gaming_lines),
		cmocka_unit_test(failed_reads),     cmocka_unit_test(game_mode_missing),
		cmocka_unit_test(unknown_features), cmocka_unit_test(product_labels),
		cmocka_unit_test(memory_sizes),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
