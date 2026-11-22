/* Original obs_enter_graphics() and obs_leave_graphics() and the real-header
 * adapters of obs-enter-leave-graphics-fields.c.h on one recording fixture
 * core. Under cargo test these adapters are the only definitions of their
 * real names, so the Rust shims reach this fixture; its graphics starts NULL
 * on every thread, so other obs-core shims that enter and leave graphics
 * (the source profiler) do nothing there, as with no graphics subsystem.
 * Every gs_enter_context (with the graphics it got) and gs_leave_context
 * is traced; the probe runs a script of calls and
 * graphics changes on the original and on a candidate pair and reports,
 * instead of aborting, any difference in the trace or a changed core, so
 * mutants are observable.
 *
 * Excluded: an uninitialized core and a dead graphics subsystem (outside the
 * original's contract) and the graphics layer itself (libobs-graphics; the
 * real enter, count and cross-thread wait stay unchanged Tier 2,
 * test_enter_leave_graphics). No intentional differences. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define ELG_TRACE 256

static _Thread_local struct obs_core elg_core;
/* Two stand-in graphics subsystems, only compared by address. */
static char elg_graphics[2];
static _Thread_local char elg_trace[ELG_TRACE];
static _Thread_local size_t elg_trace_len;

static void elg_event(char value)
{
	if (elg_trace_len + 1 < sizeof(elg_trace)) {
		elg_trace[elg_trace_len++] = value;
		elg_trace[elg_trace_len] = '\0';
	}
}

static graphics_t *elg_graphics_at(size_t index)
{
	return (graphics_t *)(void *)&elg_graphics[index];
}

/* The core. Its reads are not traced: the original reads the graphics
 * field twice in enter (the check, then the call) and the shim once, which
 * only differs under a concurrent change, outside the contract. */
static struct obs_core *elg_read_core(void)
{
	return &elg_core;
}

/* 'e' then 'A', 'B', 'N' (NULL) or '?' enters the graphics it got. */
static void elg_enter_context(graphics_t *graphics)
{
	elg_event('e');
	if (!graphics)
		elg_event('N');
	else if (graphics == elg_graphics_at(0))
		elg_event('A');
	else if (graphics == elg_graphics_at(1))
		elg_event('B');
	else
		elg_event('?');
}

/* 'l' leaves. */
static void elg_leave_context(void)
{
	elg_event('l');
}

void oracle_obs_enter_graphics(void);
void oracle_obs_leave_graphics(void);

#define gs_enter_context elg_enter_context
#define gs_leave_context elg_leave_context
#define obs elg_read_core()
#define obs_enter_graphics oracle_obs_enter_graphics
#define obs_leave_graphics oracle_obs_leave_graphics
#include "libobs/obs-enter-leave-graphics.c.h"
#undef obs_leave_graphics
#undef obs_enter_graphics
#include "enter_leave_graphics_fields.inc"
#undef obs
#undef gs_leave_context
#undef gs_enter_context

typedef void (*elg_fn)(void);

/* Runs `script` from a zeroed core: 'E' enters, 'L' leaves, '0' sets the
 * core's graphics to NULL, 'A' or 'B' to that stand-in. 'c' in the trace
 * marks a call that changed the core. Returns false for an unknown step. */
static bool elg_run(elg_fn enter, elg_fn leave, const char *script, char *trace)
{
	static _Thread_local struct obs_core before;
	memset(&elg_core, 0, sizeof(elg_core));
	elg_trace_len = 0;
	elg_trace[0] = '\0';
	for (const char *step = script; *step; step++) {
		switch (*step) {
		case '0':
			elg_core.video.graphics = NULL;
			continue;
		case 'A':
			elg_core.video.graphics = elg_graphics_at(0);
			continue;
		case 'B':
			elg_core.video.graphics = elg_graphics_at(1);
			continue;
		case 'E':
		case 'L':
			break;
		default:
			return false;
		}
		memcpy(&before, &elg_core, sizeof(before));
		(*step == 'E' ? enter : leave)();
		if (memcmp(&before, &elg_core, sizeof(before)) != 0)
			elg_event('c');
	}
	memcpy(trace, elg_trace, ELG_TRACE);
	memset(&elg_core, 0, sizeof(elg_core));
	return true;
}

/* 0: match. 2: the traces differ, 4: a call changed the core. 0x100: an
 * unknown script step. */
unsigned oracle_enter_leave_graphics_probe(elg_fn enter, elg_fn leave, const char *script);
unsigned oracle_enter_leave_graphics_probe(elg_fn enter, elg_fn leave, const char *script)
{
	static _Thread_local char original[ELG_TRACE], actual[ELG_TRACE];
	if (!elg_run(oracle_obs_enter_graphics, oracle_obs_leave_graphics, script, original) ||
	    !elg_run(enter, leave, script, actual))
		return 0x100;
	unsigned diff = 0;
	if (strcmp(original, actual) != 0)
		diff |= 2;
	if (strchr(original, 'c') || strchr(actual, 'c'))
		diff |= 4;
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void elg_copy(char *out, size_t size, const char *text)
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
bool oracle_enter_leave_graphics_original(const char *script, char *trace, size_t trace_size);
bool oracle_enter_leave_graphics_original(const char *script, char *trace, size_t trace_size)
{
	static _Thread_local char result[ELG_TRACE];
	if (!elg_run(oracle_obs_enter_graphics, oracle_obs_leave_graphics, script, result))
		return false;
	elg_copy(trace, trace_size, result);
	return true;
}

/* The adapters against the real field and the recorders. */
bool oracle_enter_leave_graphics_adapters(void);
bool oracle_enter_leave_graphics_adapters(void)
{
	memset(&elg_core, 0, sizeof(elg_core));
	elg_trace_len = 0;
	elg_trace[0] = '\0';
	bool ok = obs_enter_leave_graphics_get() == NULL;
	elg_core.video.graphics = elg_graphics_at(1);
	ok = ok && obs_enter_leave_graphics_get() == elg_graphics_at(1);
	obs_enter_leave_graphics_enter(elg_graphics_at(0));
	obs_enter_leave_graphics_leave();
	ok = ok && strcmp(elg_trace, "eAl") == 0;
	memset(&elg_core, 0, sizeof(elg_core));
	return ok;
}
