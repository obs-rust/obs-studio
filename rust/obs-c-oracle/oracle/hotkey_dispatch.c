/* Original hotkey dispatch and its real-header adapters on one recording
 * core. The probe runs the original and a candidate dispatch over the same
 * core, hotkey, callback and router, and reports every difference instead of
 * aborting, so mutants are observable.
 *
 * The adapters keep their real names here, since the Rust shim calls them.
 * Only one definition can exist in a test binary, and the binding press
 * parity worlds (binding_press_parity, press_parity) also reach the shim
 * through the C and Rust binding press count, with fake hotkeys. So an
 * adapter runs the real one on this file's core while a probe has armed it,
 * and otherwise forwards the delivered call to the per-thread fallback those
 * worlds install (rerouting off, as their fake dispatch assumed). */
#include "libobs/build/libobs.pch.h"
#include <stdlib.h>
#include <string.h>

typedef void (*dispatch_fn)(obs_hotkey_t *hotkey, bool pressed);

static _Thread_local struct obs_core dispatch_core;
static _Thread_local struct obs_core *dispatch_current;
static _Thread_local obs_hotkey_t dispatch_key;
static _Thread_local int dispatch_callback_data, dispatch_router_data;
static _Thread_local char dispatch_events[64];
static _Thread_local size_t dispatch_event_count;
static _Thread_local dispatch_fn dispatch_fallback;

static void dispatch_event(char value)
{
	if (dispatch_event_count + 1 < sizeof(dispatch_events)) {
		dispatch_events[dispatch_event_count++] = value;
		dispatch_events[dispatch_event_count] = '\0';
	}
}

/* 'o': one read of the core. The original reads it once for the flag, once
 * for the router test and twice for the routed call (func and data). */
static struct obs_core *dispatch_read_core(void)
{
	dispatch_event('o');
	return dispatch_current;
}

/* 'C'/'c': the callback for a press/release; 'X' wrong data, id or hotkey. */
static void dispatch_callback(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	if (data != &dispatch_callback_data || id != dispatch_key.id || key != &dispatch_key)
		dispatch_event('X');
	dispatch_event(pressed ? 'C' : 'c');
}

/* 'R'/'r': the router for a press/release; 'X' wrong data or id. */
static void dispatch_router(void *data, obs_hotkey_id id, bool pressed)
{
	if (data != &dispatch_router_data || id != dispatch_key.id)
		dispatch_event('X');
	dispatch_event(pressed ? 'R' : 'r');
}

void oracle_obs_hotkey_dispatch(obs_hotkey_t *hotkey, bool pressed);

#define obs dispatch_read_core()
#define obs_hotkey_dispatch oracle_obs_hotkey_dispatch
#include "libobs/obs-hotkey-dispatch.c.h"
#undef obs_hotkey_dispatch
#define obs_hotkey_dispatch_rerouting dispatch_real_rerouting
#define obs_hotkey_dispatch_has_router dispatch_real_has_router
#define obs_hotkey_dispatch_call dispatch_real_call
#define obs_hotkey_dispatch_route dispatch_real_route
#include "hotkey_dispatch_fields.inc"
#undef obs_hotkey_dispatch_rerouting
#undef obs_hotkey_dispatch_has_router
#undef obs_hotkey_dispatch_call
#undef obs_hotkey_dispatch_route
#undef obs

bool obs_hotkey_dispatch_rerouting(void);
bool obs_hotkey_dispatch_rerouting(void)
{
	return dispatch_current ? dispatch_real_rerouting() : false;
}

bool obs_hotkey_dispatch_has_router(void);
bool obs_hotkey_dispatch_has_router(void)
{
	if (!dispatch_current)
		abort();
	return dispatch_real_has_router();
}

void obs_hotkey_dispatch_call(obs_hotkey_t *hotkey, bool pressed);
void obs_hotkey_dispatch_call(obs_hotkey_t *hotkey, bool pressed)
{
	if (dispatch_current)
		dispatch_real_call(hotkey, pressed);
	else if (dispatch_fallback)
		dispatch_fallback(hotkey, pressed);
	else
		abort();
}

void obs_hotkey_dispatch_route(obs_hotkey_t *hotkey, bool pressed);
void obs_hotkey_dispatch_route(obs_hotkey_t *hotkey, bool pressed)
{
	if (!dispatch_current)
		abort();
	dispatch_real_route(hotkey, pressed);
}

/* The binding press worlds' delivery, on this thread (NULL clears it). */
void oracle_hotkey_dispatch_fallback(dispatch_fn fallback);
void oracle_hotkey_dispatch_fallback(dispatch_fn fallback)
{
	dispatch_fallback = fallback;
}

/* Bits: 1 rerouting on, 2 a router set, 4 the event is a press, 8 a NULL
 * hotkey (only where the original does not touch it: rerouting on with no
 * router). The hotkey always has its callback. */
static obs_hotkey_t *dispatch_arm(unsigned bits, obs_hotkey_id id)
{
	memset(&dispatch_core, 0, sizeof(dispatch_core));
	memset(&dispatch_key, 0, sizeof(dispatch_key));
	memset(dispatch_events, 0, sizeof(dispatch_events));
	dispatch_event_count = 0;
	dispatch_current = &dispatch_core;
	dispatch_core.hotkeys.reroute_hotkeys = (bits & 1) != 0;
	if (bits & 2) {
		dispatch_core.hotkeys.router_func = dispatch_router;
		dispatch_core.hotkeys.router_func_data = &dispatch_router_data;
	}
	dispatch_key.id = id;
	dispatch_key.func = dispatch_callback;
	dispatch_key.data = &dispatch_callback_data;
	return (bits & 8) ? NULL : &dispatch_key;
}

static void dispatch_run(dispatch_fn fn, unsigned bits, obs_hotkey_id id, char *out, size_t size)
{
	obs_hotkey_t *hotkey = dispatch_arm(bits, id);
	fn(hotkey, (bits & 4) != 0);
	/* Dispatch changes nothing it was given. */
	if (dispatch_core.hotkeys.reroute_hotkeys != ((bits & 1) != 0) ||
	    (dispatch_core.hotkeys.router_func != NULL) != ((bits & 2) != 0) || dispatch_key.id != id ||
	    dispatch_key.func != dispatch_callback || dispatch_key.data != &dispatch_callback_data ||
	    dispatch_key.pressed != 0)
		dispatch_event('M');
	dispatch_current = NULL;
	strncpy(out, dispatch_events, size - 1);
	out[size - 1] = '\0';
}

/* Nonzero when the candidate's trace differs from the original's or shows a
 * wrong argument or a mutation. */
unsigned oracle_hotkey_dispatch_probe(unsigned bits, obs_hotkey_id id, dispatch_fn candidate);
unsigned oracle_hotkey_dispatch_probe(unsigned bits, obs_hotkey_id id, dispatch_fn candidate)
{
	char original[64], other[64];
	dispatch_run(oracle_obs_hotkey_dispatch, bits, id, original, sizeof(original));
	dispatch_run(candidate, bits, id, other, sizeof(other));
	return strcmp(original, other) != 0 || strpbrk(other, "XM") != NULL;
}

/* One dispatch's trace, for exact expectations in Rust. A NULL candidate
 * runs the original. */
void oracle_hotkey_dispatch_trace(unsigned bits, obs_hotkey_id id, dispatch_fn candidate, char *out, size_t size);
void oracle_hotkey_dispatch_trace(unsigned bits, obs_hotkey_id id, dispatch_fn candidate, char *out, size_t size)
{
	dispatch_run(candidate ? candidate : oracle_obs_hotkey_dispatch, bits, id, out, size);
}

void oracle_hotkey_dispatch_layout(size_t *out);
void oracle_hotkey_dispatch_layout(size_t *out)
{
	out[0] = sizeof(obs_hotkey_id);
	out[1] = sizeof(bool);
	out[2] = sizeof(dispatch_core.hotkeys.reroute_hotkeys);
}
