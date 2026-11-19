/* C-source characterization; the external seam is reserved for the Rust swap.
 * No desktop or threads: recording hooks enforce lock/callback ordering. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core core;
static bool lock_result, locked;
static unsigned locks, unlocks, reads, calls, frees;
static char events[32];
static size_t event_count;
static obs_key_combination_t expected_combo;
static char *allocation;
static bool (*lock_callback)(obs_key_combination_t, struct dstr *);

enum output_kind { NO_OUTPUT, EMPTY_OUTPUT, TEXT_OUTPUT, PARTIAL_OUTPUT, FAILED_EMPTY, REENTER };
static enum output_kind output_kind;

static void event(char value)
{
	assert_true(event_count + 1 < sizeof(events));
	events[event_count++] = value;
	events[event_count] = '\0';
}

static struct obs_core *record_core(void)
{
	assert_true(locked);
	reads++;
	event('R');
	return &core;
}

static bool wrong_callback(obs_key_combination_t combo, struct dstr *id)
{
	(void)combo;
	(void)id;
	assert_true(false);
	return false;
}

static inline bool lock(void)
{
	assert_false(locked);
	locks++;
	event('L');
	locked = lock_result;
	/* The current callback becomes available only when locking succeeds. */
	core.hotkeys.background.shortcut_id = lock_callback;
	return lock_result;
}

static inline void unlock(void)
{
	assert_true(locked);
	assert_int_equal(reads, locks);
	unlocks++;
	event('U');
	locked = false;
	/* The wrapper must invoke its saved callback, not read this after unlock. */
	core.hotkeys.background.shortcut_id = wrong_callback;
}

static void record_free(struct dstr *id)
{
	assert_false(locked);
	frees++;
	event('F');
	assert_ptr_equal(id->array, allocation);
	dstr_free(id);
	assert_null(id->array);
	assert_int_equal(id->len, 0);
	assert_int_equal(id->capacity, 0);
}

#ifndef HOTKEY_COMBO_ID_EXTERNAL
#ifndef HOTKEY_COMBO_ID_SOURCE
#define HOTKEY_COMBO_ID_SOURCE "libobs/obs-hotkey-combination-background-id.c.h"
#endif
#define obs record_core()
#define dstr_free record_free
#include HOTKEY_COMBO_ID_SOURCE
#undef dstr_free
#undef obs
#endif

static bool callback(obs_key_combination_t combo, struct dstr *id)
{
	assert_false(locked);
	assert_int_equal(unlocks, 1);
	assert_int_equal(calls, 0);
	calls++;
	event('C');
	assert_int_equal(combo.key, expected_combo.key);
	assert_int_equal(combo.modifiers, expected_combo.modifiers);
	assert_null(id->array);
	assert_int_equal(id->len, 0);
	assert_int_equal(id->capacity, 0);
	if (output_kind == REENTER) {
		/* Reenter the public wrapper after release, with no nested callback. */
		lock_callback = NULL;
		assert_null(obs_hotkey_combination_get_background_id(combo));
	} else if (output_kind != NO_OUTPUT && output_kind != FAILED_EMPTY) {
		dstr_copy(id, output_kind == EMPTY_OUTPUT ? "" : "background-id");
		/* An empty successful result may still own an allocated empty string. */
		if (!id->array)
			id->array = bzalloc(1);
		allocation = id->array;
	}
	return output_kind != PARTIAL_OUTPUT && output_kind != FAILED_EMPTY;
}

static void query(bool success, bool with_callback, enum output_kind kind, obs_key_combination_t combo)
{
	memset(&core, 0, sizeof(core));
	memset(events, 0, sizeof(events));
	lock_result = success;
	locked = false;
	locks = unlocks = reads = calls = frees = 0;
	event_count = 0;
	allocation = NULL;
	output_kind = kind;
	expected_combo = combo;
	lock_callback = with_callback ? callback : NULL;
	core.hotkeys.background.shortcut_id = wrong_callback;
	long before = bnum_allocs();
	char *result = obs_hotkey_combination_get_background_id(combo);
	bool invoked = success && with_callback;
	bool nested = invoked && kind == REENTER;
	assert_false(locked);
	assert_int_equal(locks, nested ? 2 : 1);
	assert_int_equal(unlocks, success ? (nested ? 2 : 1) : 0);
	assert_int_equal(reads, unlocks);
	assert_int_equal(calls, invoked ? 1 : 0);
	assert_int_equal(
		frees, success && (!with_callback || kind == PARTIAL_OUTPUT || kind == FAILED_EMPTY || nested) ? 1 : 0);
	if (!success) {
		assert_string_equal(events, "L");
	} else if (!with_callback) {
		assert_string_equal(events, "LRUF");
	} else {
		assert_string_equal(events, nested                                             ? "LRUCLRUF"
					    : (kind == PARTIAL_OUTPUT || kind == FAILED_EMPTY) ? "LRUCF"
											       : "LRUC");
	}
	if (invoked && (kind == EMPTY_OUTPUT || kind == TEXT_OUTPUT)) {
		assert_ptr_equal(result, allocation);
		assert_non_null(result);
		assert_string_equal(result, kind == EMPTY_OUTPUT ? "" : "background-id");
		assert_int_equal(bnum_allocs(), before + 1);
		/* Exact C allocator ownership transfers to the caller. */
		bfree(result);
	} else {
		/* Characterized, not endorsed: callback success may return NULL. */
		assert_null(result);
	}
	assert_int_equal(bnum_allocs(), before);
}

static void lock_and_callback_contract(void **unused)
{
	(void)unused;
	const obs_key_combination_t combinations[] = {
		{0, OBS_KEY_NONE},
		{INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, OBS_KEY_A},
		{UINT32_MAX, OBS_KEY_MOUSE1},
	};
	for (size_t i = 0; i < sizeof(combinations) / sizeof(combinations[0]); i++) {
		query(false, false, NO_OUTPUT, combinations[i]);
		query(false, true, PARTIAL_OUTPUT, combinations[i]);
		query(true, false, NO_OUTPUT, combinations[i]);
		query(true, true, NO_OUTPUT, combinations[i]);
		query(true, true, EMPTY_OUTPUT, combinations[i]);
		query(true, true, TEXT_OUTPUT, combinations[i]);
		query(true, true, PARTIAL_OUTPUT, combinations[i]);
		query(true, true, FAILED_EMPTY, combinations[i]);
		query(true, true, REENTER, combinations[i]);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(lock_and_callback_contract)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
