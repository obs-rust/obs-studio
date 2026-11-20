/* Original background-state query and its real-header adapters on one
 * recording fixture. The probe runs the original and a candidate getter
 * through the same lock, field-read and unlock boundary and reports every
 * difference instead of aborting, so mutants are observable. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

typedef enum obs_hotkeys_background_state (*state_getter)(uint32_t *flags);

/* What a NULL-output run reports as its output, and what a non-NULL output
 * holds before the call. */
#define STATE_UNTOUCHED UINT32_C(0x5eed5eed)

static _Thread_local struct obs_core state_core;
static _Thread_local bool state_lock_result, state_held;
static _Thread_local enum obs_hotkeys_background_state state_value;
static _Thread_local uint32_t state_flags;
static _Thread_local uint32_t *state_output;
static _Thread_local char state_events[64];
static _Thread_local size_t state_event_count;

static void state_event(char value)
{
	if (state_event_count + 1 < sizeof(state_events)) {
		state_events[state_event_count++] = value;
		state_events[state_event_count] = '\0';
	}
}

/* 'X' marks a lock-discipline violation. */
static void state_require(bool held)
{
	if (state_held != held)
		state_event('X');
}

bool oracle_background_state_lock(void);
bool oracle_background_state_lock(void)
{
	state_require(false);
	state_event('L');
	/* 'Z': the optional output was not cleared before the lock attempt. */
	if (state_output && *state_output != 0)
		state_event('Z');
	state_held = state_lock_result;
	/* The fields are current only while the lock is held. */
	state_core.hotkeys.background.state = state_value;
	state_core.hotkeys.background.flags = state_flags;
	return state_lock_result;
}

void oracle_background_state_unlock(void);
void oracle_background_state_unlock(void)
{
	state_require(true);
	state_event('U');
	/* 'F': the flags were not stored before the lock was released. */
	if (state_output && *state_output != state_flags)
		state_event('F');
	state_held = false;
	/* A read after unlock sees other values, as well as an 'X'. */
	state_core.hotkeys.background.state = (enum obs_hotkeys_background_state) ~(unsigned)state_value;
	state_core.hotkeys.background.flags = ~state_flags;
}

static struct obs_core *state_read_core(void)
{
	state_require(true);
	state_event('R');
	return &state_core;
}

static inline bool lock(void)
{
	return oracle_background_state_lock();
}

static inline void unlock(void)
{
	oracle_background_state_unlock();
}

enum obs_hotkeys_background_state oracle_obs_hotkeys_get_background_state(uint32_t *flags);

#define obs state_read_core()
#define obs_hotkeys_get_background_state oracle_obs_hotkeys_get_background_state
#include "libobs/obs-hotkey-background-state.c.h"
#undef obs_hotkeys_get_background_state
#include "hotkey_background_state_fields.inc"
#undef obs

struct state_result {
	char events[64];
	int state;
	uint32_t output;
};

/* Bits: 1 the lock succeeds, 2 the caller passes a flags output. */
static void state_arm(unsigned bits, int value, uint32_t flags)
{
	memset(&state_core, 0, sizeof(state_core));
	memset(state_events, 0, sizeof(state_events));
	state_event_count = 0;
	state_lock_result = (bits & 1) != 0;
	state_held = false;
	state_value = (enum obs_hotkeys_background_state)value;
	state_flags = flags;
	state_output = NULL;
}

/* Arm the fixture for callers that reach the getter indirectly. */
void oracle_hotkey_background_state_arm(unsigned bits, int value, uint32_t flags);
void oracle_hotkey_background_state_arm(unsigned bits, int value, uint32_t flags)
{
	state_arm(bits, value, flags);
}

void oracle_hotkey_background_state_events(char *out, size_t size);
void oracle_hotkey_background_state_events(char *out, size_t size)
{
	strncpy(out, state_events, size - 1);
	out[size - 1] = '\0';
}

static void state_run(state_getter getter, unsigned bits, int value, uint32_t flags, struct state_result *out)
{
	state_arm(bits, value, flags);
	uint32_t output = STATE_UNTOUCHED;
	state_output = (bits & 2) ? &output : NULL;
	enum obs_hotkeys_background_state result = getter(state_output);
	state_require(false);
	memset(out, 0, sizeof(*out));
	memcpy(out->events, state_events, sizeof(out->events));
	out->state = (int)result;
	out->output = output;
	/* A mutant may return with the fixture still locked. */
	state_held = false;
	state_output = NULL;
}

/* Bits of the result: 1 trace (order, discipline, clearing or storing the
 * output), 2 returned state, 4 output value. */
unsigned oracle_hotkey_background_state_probe(unsigned bits, int value, uint32_t flags, state_getter getter);
unsigned oracle_hotkey_background_state_probe(unsigned bits, int value, uint32_t flags, state_getter getter)
{
	struct state_result original, candidate;
	state_run(oracle_obs_hotkeys_get_background_state, bits, value, flags, &original);
	state_run(getter, bits, value, flags, &candidate);
	unsigned diff = 0;
	if (strcmp(original.events, candidate.events) != 0 || strpbrk(candidate.events, "XZF"))
		diff |= 1;
	if (original.state != candidate.state)
		diff |= 2;
	if (original.output != candidate.output)
		diff |= 4;
	return diff;
}

/* One getter's trace, returned state and output, for exact expectations in
 * Rust. A NULL getter runs the original. */
int oracle_hotkey_background_state_trace(unsigned bits, int value, uint32_t flags, state_getter getter, char *out,
					 size_t size, uint32_t *output);
int oracle_hotkey_background_state_trace(unsigned bits, int value, uint32_t flags, state_getter getter, char *out,
					 size_t size, uint32_t *output)
{
	struct state_result run;
	state_run(getter ? getter : oracle_obs_hotkeys_get_background_state, bits, value, flags, &run);
	strncpy(out, run.events, size - 1);
	out[size - 1] = '\0';
	*output = run.output;
	return run.state;
}

void oracle_hotkey_background_state_layout(size_t *out);
void oracle_hotkey_background_state_layout(size_t *out)
{
	out[0] = sizeof(enum obs_hotkeys_background_state);
	out[1] = _Alignof(enum obs_hotkeys_background_state);
	out[2] = sizeof(state_core.hotkeys.background.flags);
	out[3] = OBS_HOTKEYS_BACKGROUND_NATIVE;
	out[4] = OBS_HOTKEYS_BACKGROUND_FAILED;
	out[5] = STATE_UNTOUCHED;
}
