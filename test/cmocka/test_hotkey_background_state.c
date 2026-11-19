/* Characterize the C wrapper before its Rust swap, without a desktop or threads.
 * The recording core accessor rejects eager/background reads outside the lock. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core core;
static bool lock_result, locked;
static unsigned locks, unlocks, reads;
static uint32_t *output;
static uint32_t expected_flags;
static enum obs_hotkeys_background_state expected_state;

static struct obs_core *record_core(void)
{
	assert_true(locked);
	reads++;
	return &core;
}

static inline bool lock(void)
{
	assert_false(locked);
	assert_int_equal(locks, 0);
	assert_int_equal(reads, 0);
	/* Clearing the optional output precedes even a failed lock attempt. */
	if (output)
		assert_int_equal(*output, 0);
	locks++;
	locked = lock_result;
	/* Fields become current only when the lock succeeds. */
	core.hotkeys.background.state = expected_state;
	core.hotkeys.background.flags = expected_flags;
	return lock_result;
}

static inline void unlock(void)
{
	assert_true(locked);
	assert_int_equal(locks, 1);
	assert_int_equal(unlocks, 0);
	assert_int_equal(reads, output ? 2 : 1);
	if (output)
		assert_int_equal(*output, expected_flags);
	/* The returned state must be captured before releasing the lock. */
	core.hotkeys.background.state = OBS_HOTKEYS_BACKGROUND_NATIVE;
	core.hotkeys.background.flags = 0;
	locked = false;
	unlocks++;
}

#ifndef HOTKEY_BACKGROUND_STATE_EXTERNAL
#ifndef HOTKEY_BACKGROUND_STATE_SOURCE
#define HOTKEY_BACKGROUND_STATE_SOURCE "libobs/obs-hotkey-background-state.c.h"
#endif
#define obs record_core()
#include HOTKEY_BACKGROUND_STATE_SOURCE
#undef obs
#endif

static void query(bool success, bool with_flags, enum obs_hotkeys_background_state state, uint32_t flags)
{
	memset(&core, 0, sizeof(core));
	lock_result = success;
	locked = false;
	locks = unlocks = reads = 0;
	expected_state = state;
	expected_flags = flags;
	uint32_t value = UINT32_MAX;
	output = with_flags ? &value : NULL;
	enum obs_hotkeys_background_state result = obs_hotkeys_get_background_state(output);
	assert_int_equal(result, success ? state : OBS_HOTKEYS_BACKGROUND_NATIVE);
	assert_int_equal(value, with_flags ? (success ? flags : 0) : UINT32_MAX);
	assert_int_equal(locks, 1);
	assert_int_equal(unlocks, success ? 1 : 0);
	assert_int_equal(reads, success ? (with_flags ? 2 : 1) : 0);
	assert_false(locked);
}

static void failed_lock(void **unused)
{
	(void)unused;
	query(false, true, OBS_HOTKEYS_BACKGROUND_ACTIVE, UINT32_MAX);
	query(false, false, OBS_HOTKEYS_BACKGROUND_ACTIVE, UINT32_MAX);
}

static void states_and_flags(void **unused)
{
	(void)unused;
	const enum obs_hotkeys_background_state states[] = {
		OBS_HOTKEYS_BACKGROUND_NATIVE,
		OBS_HOTKEYS_BACKGROUND_ACTIVE,
		OBS_HOTKEYS_BACKGROUND_STARTING,
		OBS_HOTKEYS_BACKGROUND_IDLE,
		OBS_HOTKEYS_BACKGROUND_WAITING,
		OBS_HOTKEYS_BACKGROUND_DECLINED,
		OBS_HOTKEYS_BACKGROUND_CLOSED,
		OBS_HOTKEYS_BACKGROUND_FAILED,
		OBS_HOTKEYS_BACKGROUND_UNAVAILABLE,
		/* Characterized, not endorsed: unknown native enum values pass through. */
		(enum obs_hotkeys_background_state)12345,
	};
	const uint32_t flags[] = {0, 1, UINT32_C(0x80000000), UINT32_C(0xa5a51234), UINT32_MAX};
	for (size_t s = 0; s < sizeof(states) / sizeof(states[0]); s++) {
		for (size_t f = 0; f < sizeof(flags) / sizeof(flags[0]); f++) {
			query(true, true, states[s], flags[f]);
			query(true, false, states[s], flags[f]);
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(failed_lock), cmocka_unit_test(states_and_flags)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
