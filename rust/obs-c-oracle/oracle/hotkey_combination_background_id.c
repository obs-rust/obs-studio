/* Original combination background-id wrapper and its real-header adapters on
 * one recording fixture. The probe runs the original and a candidate getter
 * through the same lock, callback-field, unlock and dstr_free boundary and
 * reports every difference instead of aborting, so mutants are observable. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

typedef char *(*combo_id_getter)(obs_key_combination_t combo);

enum combo_id_output { NO_OUTPUT, EMPTY_OUTPUT, TEXT_OUTPUT, PARTIAL_OUTPUT, FAILED_EMPTY, REENTER, OUTPUT_KINDS };

static _Thread_local struct obs_core combo_id_core;
static _Thread_local bool combo_id_lock_result, combo_id_held, combo_id_with_callback;
static _Thread_local enum combo_id_output combo_id_kind;
static _Thread_local obs_key_combination_t combo_id_expected;
static _Thread_local combo_id_getter combo_id_current;
static _Thread_local char *combo_id_allocation;
static _Thread_local char combo_id_events[64];
static _Thread_local size_t combo_id_event_count;

static void combo_id_event(char value)
{
	if (combo_id_event_count + 1 < sizeof(combo_id_events)) {
		combo_id_events[combo_id_event_count++] = value;
		combo_id_events[combo_id_event_count] = '\0';
	}
}

/* 'X' marks a lock-discipline violation. */
static void combo_id_require(bool held)
{
	if (combo_id_held != held)
		combo_id_event('X');
}

static bool combo_id_wrong_callback(obs_key_combination_t combo, struct dstr *id)
{
	(void)combo;
	(void)id;
	/* The field after unlock: a wrapper must invoke its saved callback. */
	combo_id_event('W');
	return false;
}

static bool combo_id_callback(obs_key_combination_t combo, struct dstr *id)
{
	combo_id_require(false);
	combo_id_event('C');
	if (combo.key != combo_id_expected.key || combo.modifiers != combo_id_expected.modifiers)
		combo_id_event('K');
	if (id->array || id->len || id->capacity)
		combo_id_event('Z');
	if (combo_id_kind == REENTER) {
		/* Reenter the getter under test after release, with no callback. */
		combo_id_with_callback = false;
		if (combo_id_current(combo))
			combo_id_event('N');
	} else if (combo_id_kind != NO_OUTPUT && combo_id_kind != FAILED_EMPTY) {
		dstr_copy(id, combo_id_kind == EMPTY_OUTPUT ? "" : "background-id");
		/* An empty successful result may still own an allocated empty string. */
		if (!id->array)
			id->array = bzalloc(1);
		combo_id_allocation = id->array;
	}
	return combo_id_kind != PARTIAL_OUTPUT && combo_id_kind != FAILED_EMPTY;
}

bool oracle_combo_id_lock(void);
bool oracle_combo_id_lock(void)
{
	combo_id_require(false);
	combo_id_event('L');
	combo_id_held = combo_id_lock_result;
	/* The current callback is visible only while the lock is held. */
	combo_id_core.hotkeys.background.shortcut_id = combo_id_with_callback ? combo_id_callback : NULL;
	return combo_id_lock_result;
}

void oracle_combo_id_unlock(void);
void oracle_combo_id_unlock(void)
{
	combo_id_require(true);
	combo_id_event('U');
	combo_id_held = false;
	combo_id_core.hotkeys.background.shortcut_id = combo_id_wrong_callback;
}

static struct obs_core *combo_id_read_core(void)
{
	combo_id_require(true);
	combo_id_event('R');
	return &combo_id_core;
}

static void combo_id_free(struct dstr *id)
{
	combo_id_require(false);
	combo_id_event('F');
	if (id->array != combo_id_allocation)
		combo_id_event('P');
	dstr_free(id);
	if (id->array || id->len || id->capacity)
		combo_id_event('P');
}

static inline bool lock(void)
{
	return oracle_combo_id_lock();
}

static inline void unlock(void)
{
	oracle_combo_id_unlock();
}

char *oracle_obs_hotkey_combination_get_background_id(obs_key_combination_t combo);

#define obs combo_id_read_core()
#define dstr_free combo_id_free
#define obs_hotkey_seam_lock oracle_combo_id_lock
#define obs_hotkey_seam_unlock oracle_combo_id_unlock
#define obs_hotkey_combination_get_background_id oracle_obs_hotkey_combination_get_background_id
#include "libobs/obs-hotkey-combination-background-id.c.h"
#undef obs_hotkey_combination_get_background_id
#include "hotkey_combination_background_id_fields.inc"
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock
#undef dstr_free
#undef obs

struct combo_id_result {
	char events[64];
	bool null_result, transferred;
	char text[16];
};

static void combo_id_arm(combo_id_getter getter, unsigned bits, obs_key_combination_t combo)
{
	memset(&combo_id_core, 0, sizeof(combo_id_core));
	memset(combo_id_events, 0, sizeof(combo_id_events));
	combo_id_event_count = 0;
	combo_id_lock_result = (bits & 1) != 0;
	combo_id_with_callback = (bits & 2) != 0;
	combo_id_kind = (enum combo_id_output)((bits >> 2) % OUTPUT_KINDS);
	combo_id_held = false;
	combo_id_expected = combo;
	combo_id_current = getter;
	combo_id_allocation = NULL;
	combo_id_core.hotkeys.background.shortcut_id = combo_id_wrong_callback;
}

/* Arm the fixture for callers that reach the getter indirectly. */
void oracle_hotkey_combination_background_id_arm(unsigned bits, uint32_t modifiers, int key, combo_id_getter getter);
void oracle_hotkey_combination_background_id_arm(unsigned bits, uint32_t modifiers, int key, combo_id_getter getter)
{
	obs_key_combination_t combo = {modifiers, (obs_key_t)key};
	combo_id_arm(getter, bits, combo);
}

void oracle_hotkey_combination_background_id_events(char *out, size_t size);
void oracle_hotkey_combination_background_id_events(char *out, size_t size)
{
	strncpy(out, combo_id_events, size - 1);
	out[size - 1] = '\0';
}

static void combo_id_run(combo_id_getter getter, unsigned bits, obs_key_combination_t combo,
			 struct combo_id_result *out)
{
	combo_id_arm(getter, bits, combo);
	char *result = getter(combo);
	combo_id_require(false);
	memset(out, 0, sizeof(*out));
	memcpy(out->events, combo_id_events, sizeof(out->events));
	out->null_result = !result;
	out->transferred = result && result == combo_id_allocation;
	if (result)
		strncpy(out->text, result, sizeof(out->text) - 1);
	bfree(result);
	/* A mutant may return with the fixture still locked. */
	combo_id_held = false;
}

/* Bits of the result: 1 trace (order, discipline, callback input or partial
 * free), 2 returned pointer or text. Allocation balance follows from the
 * trace: every non-transferred allocation is freed exactly once (F, P). */
unsigned oracle_hotkey_combination_background_id_probe(unsigned bits, uint32_t modifiers, int key,
						       combo_id_getter getter);
unsigned oracle_hotkey_combination_background_id_probe(unsigned bits, uint32_t modifiers, int key,
						       combo_id_getter getter)
{
	obs_key_combination_t combo = {modifiers, (obs_key_t)key};
	struct combo_id_result original, candidate;
	combo_id_run(oracle_obs_hotkey_combination_get_background_id, bits, combo, &original);
	combo_id_run(getter, bits, combo, &candidate);
	unsigned diff = 0;
	if (strcmp(original.events, candidate.events) != 0 || strpbrk(candidate.events, "XWKZPN"))
		diff |= 1;
	if (original.null_result != candidate.null_result || original.transferred != candidate.transferred ||
	    strcmp(original.text, candidate.text) != 0)
		diff |= 2;
	return diff;
}

/* One getter's trace, for exact expectations in Rust. */
void oracle_hotkey_combination_background_id_trace(unsigned bits, combo_id_getter getter, char *out, size_t size);
void oracle_hotkey_combination_background_id_trace(unsigned bits, combo_id_getter getter, char *out, size_t size)
{
	struct combo_id_result run;
	obs_key_combination_t combo = {INTERACT_CONTROL_KEY, OBS_KEY_A};
	combo_id_run(getter ? getter : oracle_obs_hotkey_combination_get_background_id, bits, combo, &run);
	strncpy(out, run.events, size - 1);
	out[size - 1] = '\0';
}

void oracle_hotkey_combination_background_id_layout(size_t *out);
void oracle_hotkey_combination_background_id_layout(size_t *out)
{
	out[0] = sizeof(struct dstr);
	out[1] = _Alignof(struct dstr);
	out[2] = offsetof(struct dstr, array);
	out[3] = offsetof(struct dstr, len);
	out[4] = offsetof(struct dstr, capacity);
	out[5] = sizeof(obs_key_combination_t);
	out[6] = _Alignof(obs_key_combination_t);
	out[7] = offsetof(obs_key_combination_t, modifiers);
	out[8] = offsetof(obs_key_combination_t, key);
	out[9] = sizeof(combo_id_core.hotkeys.background.shortcut_id);
}
