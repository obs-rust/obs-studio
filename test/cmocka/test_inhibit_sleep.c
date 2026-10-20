/* Characterization of os_inhibit_sleep_* on Linux and FreeBSD
 * (libobs/util/platform-nix-inhibit.c) before its Rust port (#1500), for the
 * path without a desktop inhibitor: no session D-Bus, so neither the portal
 * (platform-nix-portal.c) nor the session D-Bus inhibitor
 * (platform-nix-dbus.c) is found, and libobs falls back to a thread that runs
 * "xdg-screensaver reset" every 30 s. test_inhibit_sleep_dbus covers the
 * portal and D-Bus paths on a private bus.
 *
 * DBUS_SESSION_BUS_ADDRESS points at a socket that does not exist, and PATH
 * holds only a fake xdg-screensaver that records each run in a marker file,
 * so the user's session bus and screensaver are never touched. Everything
 * lives under a temp dir relative to cwd. No sleeps: the cases only check
 * return values, that deactivating and destroying join the thread at once,
 * and that the first reset waits the full 30 s (the fake never runs here).
 *
 * Not characterized: the Flatpak branch (create returns NULL when the portal
 * is missing and /.flatpak-info exists) needs a file at the root of the file
 * system. Cases marked "characterized, not endorsed" are odd but real. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <unistd.h>
#include <sys/stat.h>

#include <util/c99defs.h>
#include <util/platform.h>

#define ROOT "test_inhibit_sleep.tmp"
#define FAKE ROOT "/xdg-screensaver"
#define MARKER ROOT "/reset-ran"

/* Deactivating or destroying signals the thread and joins it; anything
 * near the 30 s reset interval means it waited instead. */
#define JOIN_LIMIT_NS (5ULL * 1000000000ULL)

static char root_abs[PATH_MAX];

static bool marker_exists(void)
{
	return access(MARKER, F_OK) == 0;
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	unlink(FAKE);
	unlink(MARKER);
	rmdir(ROOT);
	if (mkdir(ROOT, 0755) != 0)
		return -1;
	if (!realpath(ROOT, root_abs))
		return -1;

	FILE *f = fopen(FAKE, "w");
	if (!f)
		return -1;
	fprintf(f, "#!/bin/sh\necho \"$@\" >> '%s/reset-ran'\n", root_abs);
	fclose(f);
	if (chmod(FAKE, 0755) != 0)
		return -1;

	/* No session bus: g_bus_get_sync() fails at once for both GIO
	 * inhibitors, and nothing autolaunches one. */
	char bus[PATH_MAX + 32];
	snprintf(bus, sizeof(bus), "unix:path=%s/no-such-bus", root_abs);
	setenv("DBUS_SESSION_BUS_ADDRESS", bus, 1);
	unsetenv("DBUS_STARTER_ADDRESS");
	setenv("PATH", root_abs, 1);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);

	unlink(FAKE);
	unlink(MARKER);
	rmdir(ROOT);
	return 0;
}

/* A NULL inhibitor is refused, and destroying NULL does nothing. */
static void null_info_is_rejected(void **state)
{
	UNUSED_PARAMETER(state);

	assert_false(os_inhibit_sleep_set_active(NULL, true));
	assert_false(os_inhibit_sleep_set_active(NULL, false));
	os_inhibit_sleep_destroy(NULL);
}

/* Without a bus, create still returns an inhibitor (the xdg-screensaver
 * fallback). set_active returns true only when the state changes. */
static void no_bus_falls_back_to_xdg_screensaver(void **state)
{
	UNUSED_PARAMETER(state);

	os_inhibit_t *info = os_inhibit_sleep_create("OBS Video/audio");
	assert_non_null(info);

	/* A new inhibitor is inactive, so deactivating it is no change. */
	assert_false(os_inhibit_sleep_set_active(info, false));

	assert_true(os_inhibit_sleep_set_active(info, true));
	assert_false(os_inhibit_sleep_set_active(info, true));

	uint64_t start = os_gettime_ns();
	assert_true(os_inhibit_sleep_set_active(info, false));
	assert_true(os_gettime_ns() - start < JOIN_LIMIT_NS);
	assert_false(os_inhibit_sleep_set_active(info, false));

	os_inhibit_sleep_destroy(info);

	/* The thread waits 30 s before its first reset. */
	assert_false(marker_exists());
}

/* Every activate starts a new thread and every deactivate joins it. */
static void repeated_activate_deactivate(void **state)
{
	UNUSED_PARAMETER(state);

	os_inhibit_t *info = os_inhibit_sleep_create("OBS Video/audio");
	assert_non_null(info);

	uint64_t start = os_gettime_ns();
	for (int i = 0; i < 50; i++) {
		assert_true(os_inhibit_sleep_set_active(info, true));
		assert_false(os_inhibit_sleep_set_active(info, true));
		assert_true(os_inhibit_sleep_set_active(info, false));
		assert_false(os_inhibit_sleep_set_active(info, false));
	}
	assert_true(os_gettime_ns() - start < JOIN_LIMIT_NS);

	os_inhibit_sleep_destroy(info);
	assert_false(marker_exists());
}

/* Destroy deactivates first: it stops and joins a running thread. */
static void destroy_while_active_joins(void **state)
{
	UNUSED_PARAMETER(state);

	os_inhibit_t *info = os_inhibit_sleep_create("OBS Video/audio");
	assert_non_null(info);
	assert_true(os_inhibit_sleep_set_active(info, true));

	uint64_t start = os_gettime_ns();
	os_inhibit_sleep_destroy(info);
	assert_true(os_gettime_ns() - start < JOIN_LIMIT_NS);
	assert_false(marker_exists());
}

/* Create then destroy, never activated. */
static void destroy_inactive(void **state)
{
	UNUSED_PARAMETER(state);

	for (int i = 0; i < 10; i++) {
		os_inhibit_t *info = os_inhibit_sleep_create("OBS Video/audio");
		assert_non_null(info);
		os_inhibit_sleep_destroy(info);
	}
}

/* The reason is only copied in this path, never parsed: NULL, empty, long
 * and non-ASCII reasons all work the same. NULL is accepted here
 * (characterized, not endorsed): with a portal or D-Bus inhibitor it would
 * reach g_variant_new_string(NULL), so test_inhibit_sleep_dbus does not pin
 * it. */
static void null_and_odd_reasons(void **state)
{
	UNUSED_PARAMETER(state);

	char long_reason[4097];
	memset(long_reason, 'r', sizeof(long_reason) - 1);
	long_reason[sizeof(long_reason) - 1] = '\0';

	const char *reasons[] = {
		NULL, "", "Aufnahme l\xC3\xA4uft \xE2\x80\x93 \xE2\x9C\x93", "line\nbreak\ttab", long_reason,
	};

	for (size_t i = 0; i < sizeof(reasons) / sizeof(reasons[0]); i++) {
		os_inhibit_t *info = os_inhibit_sleep_create(reasons[i]);
		assert_non_null(info);
		assert_true(os_inhibit_sleep_set_active(info, true));
		assert_true(os_inhibit_sleep_set_active(info, false));
		os_inhibit_sleep_destroy(info);
	}
}

/* The reason is copied at create: freeing the caller's string before use
 * changes nothing. test_inhibit_sleep_dbus sees the copy on the bus; here
 * it only must not be read again. */
static void reason_is_copied(void **state)
{
	UNUSED_PARAMETER(state);

	char *reason = strdup("OBS Video/audio");
	assert_non_null(reason);
	os_inhibit_t *info = os_inhibit_sleep_create(reason);
	memset(reason, 'x', strlen(reason));
	free(reason);

	assert_non_null(info);
	assert_true(os_inhibit_sleep_set_active(info, true));
	os_inhibit_sleep_destroy(info);
}

/* Each inhibitor has its own state and thread. */
static void two_inhibitors_are_independent(void **state)
{
	UNUSED_PARAMETER(state);

	os_inhibit_t *a = os_inhibit_sleep_create("a");
	os_inhibit_t *b = os_inhibit_sleep_create("b");
	assert_non_null(a);
	assert_non_null(b);
	assert_ptr_not_equal(a, b);

	assert_true(os_inhibit_sleep_set_active(a, true));
	assert_true(os_inhibit_sleep_set_active(b, true));
	assert_true(os_inhibit_sleep_set_active(a, false));
	assert_false(os_inhibit_sleep_set_active(b, true));
	assert_false(os_inhibit_sleep_set_active(a, false));

	os_inhibit_sleep_destroy(a);
	assert_true(os_inhibit_sleep_set_active(b, false));
	os_inhibit_sleep_destroy(b);
	assert_false(marker_exists());
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_info_is_rejected),
		cmocka_unit_test(no_bus_falls_back_to_xdg_screensaver),
		cmocka_unit_test(repeated_activate_deactivate),
		cmocka_unit_test(destroy_while_active_joins),
		cmocka_unit_test(destroy_inactive),
		cmocka_unit_test(null_and_odd_reasons),
		cmocka_unit_test(reason_is_copied),
		cmocka_unit_test(two_inhibitors_are_independent),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
