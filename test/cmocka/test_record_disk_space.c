/*
 * Recording with the Custom Output (FFmpeg) to a URL must not be stopped for
 * low disk space (upstream obsproject/obs-studio#12952, reported by 1A3Dev).
 *
 * A recording to a URL writes nothing to disk, so the recording folder
 * setting may be empty or point nowhere, and os_get_free_disk_space() returns
 * 0 for it. obsproject/obs-studio#10454 skipped the check when such a
 * recording starts, but the check made every second while recording
 * (OBSBasic::CheckDiskSpaceRemaining) kept it: about a second in, the
 * recording was stopped with "Recording stopped because of low disk space"
 * and the "Insufficient disk space" message.
 *
 * The test builds the Qt-free check the frontend makes
 * (frontend/utility/record-disk-space.c) with a stub for the free space, so
 * it needs neither Qt nor a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include "record-disk-space.h"

#define MB (1024ULL * 1024ULL)

static uint64_t free_bytes;
static int calls;

static uint64_t stub_free_space(const char *path)
{
	(void)path;
	calls++;
	return free_bytes;
}

static bool low(enum record_disk_check when, bool to_url, const char *path, uint64_t bytes)
{
	free_bytes = bytes;
	calls = 0;
	return record_disk_space_low(when, to_url, path, stub_free_space);
}

/* The report: a recording to a URL, the folder setting empty (no free space
 * found), checked while it runs. */
static void url_recording_not_stopped_while_running(void **state)
{
	(void)state;
	assert_false(low(RECORD_DISK_CHECK_RUNNING, true, "", 0));
	assert_false(low(RECORD_DISK_CHECK_RUNNING, true, "/nonexistent", 0));
	/* nothing on disk to look at */
	assert_int_equal(calls, 0);
}

/* Guard: starting one was fixed by obsproject/obs-studio#10454. */
static void url_recording_starts(void **state)
{
	(void)state;
	assert_false(low(RECORD_DISK_CHECK_START, true, "", 0));
}

/* Guards: a recording to a file is still refused, and stopped, when the disk
 * is nearly full, and runs when it is not. */
static void file_recording_low_disk_stops(void **state)
{
	(void)state;
	assert_true(low(RECORD_DISK_CHECK_START, false, "/rec", 10 * MB));
	assert_true(low(RECORD_DISK_CHECK_RUNNING, false, "/rec", 10 * MB));
	assert_true(low(RECORD_DISK_CHECK_RUNNING, false, "/rec", 50 * MB - 1));
}

static void file_recording_enough_space_runs(void **state)
{
	(void)state;
	assert_false(low(RECORD_DISK_CHECK_START, false, "/rec", 50 * MB));
	assert_false(low(RECORD_DISK_CHECK_RUNNING, false, "/rec", 100 * 1024 * MB));
}

static void no_path_not_checked(void **state)
{
	(void)state;
	assert_false(low(RECORD_DISK_CHECK_RUNNING, false, NULL, 0));
	assert_int_equal(calls, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(url_recording_not_stopped_while_running),
		cmocka_unit_test(url_recording_starts),
		cmocka_unit_test(file_recording_low_disk_stops),
		cmocka_unit_test(file_recording_enough_space_runs),
		cmocka_unit_test(no_path_not_checked),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
