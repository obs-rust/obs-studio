/* Original obs_hotkeys_background_status_changed() and the real-header
 * adapters of obs-hotkeys-background-status-changed-fields.c.h on one
 * recording fixture core. Under cargo test these adapters are the only
 * definitions of their real names, so the Rust shim reaches this fixture.
 * Every signal is recorded with its handler, its name and its calldata; a
 * scripted callback may replace the handler, notify again through the
 * function under test, or remove the core, as the unchanged C
 * characterization does. The probe runs the original and a candidate on the
 * same script and reports any difference in the trace or the state left
 * behind, instead of aborting, so mutants are observable. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define HB_HANDLERS 5
#define HB_TRACE 128
#define HB_SCRIPT 32

/* Callback actions, one per signal in order. */
enum hb_action {
	HB_NOTHING = 0,
	/* 1..4: install that handler, then notify again. */
	HB_REMOVE_CORE = 5,
	HB_CLEAR_HANDLER = 6,
	/* 7..10: install handler (action - 6) without notifying again. */
};

typedef void (*hb_notify_fn)(void);

static _Thread_local struct obs_core hb_core;
static _Thread_local struct obs_core *hb_current;
static _Thread_local char hb_tokens[HB_HANDLERS];
static _Thread_local hb_notify_fn hb_function;
static _Thread_local uint8_t hb_script[HB_SCRIPT];
static _Thread_local size_t hb_script_len, hb_script_pos;
static _Thread_local char hb_trace[HB_TRACE];
static _Thread_local size_t hb_trace_len;

static void hb_event(char value)
{
	if (hb_trace_len + 1 < sizeof(hb_trace)) {
		hb_trace[hb_trace_len++] = value;
		hb_trace[hb_trace_len] = '\0';
	}
}

static signal_handler_t *hb_handler(uint8_t index)
{
	return index && index < HB_HANDLERS ? (signal_handler_t *)&hb_tokens[index] : NULL;
}

static char hb_name(const signal_handler_t *handler)
{
	if (!handler)
		return '-';
	for (size_t i = 1; i < HB_HANDLERS; i++) {
		if (handler == (const signal_handler_t *)&hb_tokens[i])
			return (char)('0' + i);
	}
	return '?';
}

/* Each signal: its handler's name, 'N' for a wrong signal name, 'D' for
 * calldata, then the callback's action. */
static void hb_record(signal_handler_t *handler, const char *name, calldata_t *data)
{
	hb_event(hb_name(handler));
	if (!name || strcmp(name, "hotkey_background_status_changed") != 0)
		hb_event('N');
	if (data)
		hb_event('D');
	uint8_t action = hb_script_pos < hb_script_len ? hb_script[hb_script_pos++] : HB_NOTHING;
	if (action >= 1 && action <= 4) {
		hb_event('(');
		hb_core.hotkeys.signals = hb_handler(action);
		hb_function();
		hb_event(')');
	} else if (action == HB_REMOVE_CORE) {
		hb_current = NULL;
	} else if (action == HB_CLEAR_HANDLER) {
		hb_core.hotkeys.signals = NULL;
	} else if (action >= 7 && action <= 10) {
		hb_core.hotkeys.signals = hb_handler((uint8_t)(action - 6));
	}
}

void oracle_obs_hotkeys_background_status_changed(void);

#define obs hb_current
#define signal_handler_signal hb_record
#define obs_hotkeys_background_status_changed oracle_obs_hotkeys_background_status_changed
#include "libobs/obs-hotkeys-background-status-changed.c.h"
#undef obs_hotkeys_background_status_changed
#include "hotkeys_background_status_changed_fields.inc"
#undef signal_handler_signal
#undef obs

struct hb_result {
	char trace[HB_TRACE];
	bool core;
	char handler;
};

/* `calls` entries: bit 7 set for a present core, the low bits the handler
 * (0 for none) installed before that call. `script` holds the callback
 * actions consumed by the signals in order. */
static void hb_run(hb_notify_fn notify, const uint8_t *calls, size_t count, const uint8_t *script, size_t script_len,
		   struct hb_result *out)
{
	memset(&hb_core, 0, sizeof(hb_core));
	memcpy(hb_script, script, script_len);
	hb_script_len = script_len;
	hb_script_pos = 0;
	hb_trace_len = 0;
	hb_trace[0] = '\0';
	hb_function = notify;
	for (size_t i = 0; i < count; i++) {
		hb_current = (calls[i] & 0x80) ? &hb_core : NULL;
		hb_core.hotkeys.signals = hb_handler(calls[i] & 0x7f);
		hb_event('|');
		notify();
	}
	memcpy(out->trace, hb_trace, sizeof(out->trace));
	out->core = hb_current == &hb_core;
	out->handler = hb_name(hb_core.hotkeys.signals);
	hb_current = NULL;
	hb_function = NULL;
}

/* 0: match. 1: the traces differ, 2: the state left differs, 0x100: bad
 * arguments. */
unsigned oracle_hotkeys_background_status_probe(hb_notify_fn candidate, const uint8_t *calls, size_t count,
						const uint8_t *script, size_t script_len);
unsigned oracle_hotkeys_background_status_probe(hb_notify_fn candidate, const uint8_t *calls, size_t count,
						const uint8_t *script, size_t script_len)
{
	static _Thread_local struct hb_result original, actual;
	if (script_len > HB_SCRIPT)
		return 0x100;
	hb_run(oracle_obs_hotkeys_background_status_changed, calls, count, script, script_len, &original);
	hb_run(candidate, calls, count, script, script_len, &actual);
	unsigned diff = 0;
	if (strcmp(original.trace, actual.trace) != 0)
		diff |= 1;
	if (original.core != actual.core || original.handler != actual.handler)
		diff |= 2;
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void hb_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* The original's trace, for the characterization checks. */
void oracle_hotkeys_background_status_original(const uint8_t *calls, size_t count, const uint8_t *script,
					       size_t script_len, char *trace, size_t trace_size);
void oracle_hotkeys_background_status_original(const uint8_t *calls, size_t count, const uint8_t *script,
					       size_t script_len, char *trace, size_t trace_size)
{
	static _Thread_local struct hb_result result;
	if (script_len > HB_SCRIPT) {
		hb_copy(trace, trace_size, "");
		return;
	}
	hb_run(oracle_obs_hotkeys_background_status_changed, calls, count, script, script_len, &result);
	hb_copy(trace, trace_size, result.trace);
}

/* The adapters against the real fields: no core, a core without a handler,
 * a core with one, and the signal passing its handler and name with NULL
 * calldata. */
bool oracle_hotkeys_background_status_adapters(void);
bool oracle_hotkeys_background_status_adapters(void)
{
	memset(&hb_core, 0, sizeof(hb_core));
	hb_script_len = 0;
	hb_script_pos = 0;
	hb_trace_len = 0;
	hb_trace[0] = '\0';
	hb_current = NULL;
	bool ok = obs_hotkeys_background_status_signals() == NULL;
	hb_current = &hb_core;
	ok = ok && obs_hotkeys_background_status_signals() == NULL;
	hb_core.hotkeys.signals = hb_handler(3);
	ok = ok && obs_hotkeys_background_status_signals() == hb_handler(3);
	obs_hotkeys_background_status_signal(hb_handler(2), "hotkey_background_status_changed");
	ok = ok && strcmp(hb_trace, "2") == 0;
	hb_current = NULL;
	return ok;
}
