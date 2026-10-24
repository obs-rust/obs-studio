#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-source-media-admission.h"

/*
 * Characterization of the media action admission decisions of
 * libobs/obs-source.c, moved to libobs/obs-source-media-admission.c before
 * its Rust port:
 * - obs_source_media_admit(), _admit_flags() and _admit_callback(): whether
 *   each obs_source_media_*() call is accepted, one step at a time (valid,
 *   OBS_SOURCE_CONTROLLABLE_MEDIA, then the call's callback for every call
 *   but _started() and _ended()), and which value each step reads next;
 * - obs_source_media_time_fallback() and _state_fallback(): what the
 *   getters return when a call is rejected;
 * - obs_source_media_dispatch(), _signal_reads_pause() and _signal(): what
 *   process_media_actions() does with each queued action, the callback it
 *   calls and the signal it emits after it.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_media_play_after_stop,
 * test_media_restart_first_frame, test_media_restart_stale_frame,
 * test_media_seek_keyframe, test_media_time, test_slideshow_manual_end and
 * test_stinger_media_duration cover the callers through the real libobs.
 */

enum { REJECT = OBS_SOURCE_MEDIA_ADMIT_REJECT, ACCEPT = OBS_SOURCE_MEDIA_ADMIT_ACCEPT };
enum { READ_FLAGS = OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS, READ_CALLBACK = OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK };

#define CALL_COUNT 11

/* output flags: none, the flag alone, every bit, every bit but it, its
 * neighbours alone, and the flag with others */
static const uint32_t FLAGS[] = {0,
				 OBS_SOURCE_CONTROLLABLE_MEDIA,
				 UINT32_MAX,
				 ~(uint32_t)OBS_SOURCE_CONTROLLABLE_MEDIA,
				 (uint32_t)OBS_SOURCE_CONTROLLABLE_MEDIA << 1,
				 (uint32_t)OBS_SOURCE_CONTROLLABLE_MEDIA >> 1,
				 OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO | OBS_SOURCE_ASYNC,
				 OBS_SOURCE_CONTROLLABLE_MEDIA | OBS_SOURCE_ASYNC_VIDEO | OBS_SOURCE_AUDIO};
#define FLAG_COUNT (sizeof(FLAGS) / sizeof(FLAGS[0]))

/* calls, action types and dispatches outside their enums */
static const int OUTSIDE_CALLS[] = {-1, 11, 12, 99, INT_MIN, INT_MAX};
#define OUTSIDE_CALL_COUNT (sizeof(OUTSIDE_CALLS) / sizeof(OUTSIDE_CALLS[0]))
static const int OUTSIDE_TYPES[] = {7, 8, -1, 100, INT_MIN, INT_MAX};
#define OUTSIDE_TYPE_COUNT (sizeof(OUTSIDE_TYPES) / sizeof(OUTSIDE_TYPES[0]))
static const int OUTSIDE_DISPATCHES[] = {8, 9, -1, 99, INT_MIN, INT_MAX};
#define OUTSIDE_DISPATCH_COUNT (sizeof(OUTSIDE_DISPATCHES) / sizeof(OUTSIDE_DISPATCHES[0]))

static void check_int(long long actual, long long expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %lld, expected %lld\n", line, what, actual, expected);
		fail();
	}
}

static void check_bool(bool actual, bool expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %s, expected %s\n", line, what, actual ? "true" : "false",
			expected ? "true" : "false");
		fail();
	}
}

static void check_str(const char *actual, const char *expected, const char *what, int line)
{
	if ((actual == NULL) != (expected == NULL) || (actual && strcmp(actual, expected) != 0)) {
		fprintf(stderr, "line %d: %s is %s, expected %s\n", line, what, actual ? actual : "NULL",
			expected ? expected : "NULL");
		fail();
	}
}

#define CHECK_INT(actual, expected) check_int((long long)(actual), (long long)(expected), #actual, __LINE__)
#define CHECK_BOOL(actual, expected) check_bool((actual), (expected), #actual, __LINE__)
#define CHECK_STR(actual, expected) check_str((actual), (expected), #actual, __LINE__)

/* The callers' rule, written out: the calls that need a callback. */
static bool needs_callback(int call)
{
	return call != OBS_SOURCE_MEDIA_CALL_STARTED && call != OBS_SOURCE_MEDIA_CALL_ENDED;
}

/* --- enum values ------------------------------------------------------- */

static void enum_values(void **state)
{
	(void)state;

	/* the mirrors obs-source.c checks against libobs */
	CHECK_INT(OBS_SOURCE_MEDIA_CONTROLLABLE, OBS_SOURCE_CONTROLLABLE_MEDIA);
	CHECK_INT(OBS_SOURCE_MEDIA_CONTROLLABLE, 1u << 13);
	CHECK_INT(OBS_SOURCE_MEDIA_STATE_NONE, OBS_MEDIA_STATE_NONE);
	CHECK_INT(OBS_SOURCE_MEDIA_STATE_NONE, 0);
	CHECK_INT(OBS_SOURCE_MEDIA_ACTION_NONE, 0);
	CHECK_INT(OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE, 1);
	CHECK_INT(OBS_SOURCE_MEDIA_ACTION_RESTART, 2);
	CHECK_INT(OBS_SOURCE_MEDIA_ACTION_STOP, 3);
	CHECK_INT(OBS_SOURCE_MEDIA_ACTION_NEXT, 4);
	CHECK_INT(OBS_SOURCE_MEDIA_ACTION_PREVIOUS, 5);
	CHECK_INT(OBS_SOURCE_MEDIA_ACTION_SET_TIME, 6);

	CHECK_INT(OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE, 0);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_RESTART, 1);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_STOP, 2);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_NEXT, 3);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_PREVIOUS, 4);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_GET_DURATION, 5);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_GET_TIME, 6);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_SET_TIME, 7);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_GET_STATE, 8);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_STARTED, 9);
	CHECK_INT(OBS_SOURCE_MEDIA_CALL_ENDED, 10);

	CHECK_INT(OBS_SOURCE_MEDIA_ADMIT_REJECT, 0);
	CHECK_INT(OBS_SOURCE_MEDIA_ADMIT_ACCEPT, 1);
	CHECK_INT(OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS, 2);
	CHECK_INT(OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK, 3);

	CHECK_INT(OBS_SOURCE_MEDIA_DISPATCH_RETURN, 0);
	CHECK_INT(OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE, 1);
	CHECK_INT(OBS_SOURCE_MEDIA_DISPATCH_RESTART, 2);
	CHECK_INT(OBS_SOURCE_MEDIA_DISPATCH_STOP, 3);
	CHECK_INT(OBS_SOURCE_MEDIA_DISPATCH_NEXT, 4);
	CHECK_INT(OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS, 5);
	CHECK_INT(OBS_SOURCE_MEDIA_DISPATCH_SET_TIME, 6);
	CHECK_INT(OBS_SOURCE_MEDIA_DISPATCH_SKIP, 7);

	/* the enums cross as int */
	CHECK_INT(sizeof(enum obs_source_media_call), sizeof(int));
	CHECK_INT(sizeof(enum obs_source_media_admit), sizeof(int));
	CHECK_INT(sizeof(enum obs_source_media_dispatch), sizeof(int));
	CHECK_INT(sizeof(enum obs_media_state), sizeof(int));
}

/* --- admission --------------------------------------------------------- */

static void admit_valid(void **state)
{
	(void)state;

	/* an invalid source (NULL, or no data for every call but _started()
	 * and _ended()) is rejected before its flags are read */
	CHECK_INT(obs_source_media_admit(false), REJECT);
	CHECK_INT(obs_source_media_admit(true), READ_FLAGS);
}

static void admit_flags_every_call(void **state)
{
	(void)state;

	for (int call = 0; call < CALL_COUNT; call++)
		for (size_t f = 0; f < FLAG_COUNT; f++) {
			const bool controllable = (FLAGS[f] & OBS_SOURCE_CONTROLLABLE_MEDIA) != 0;
			const int expected = !controllable ? REJECT : needs_callback(call) ? READ_CALLBACK : ACCEPT;
			CHECK_INT(obs_source_media_admit_flags((enum obs_source_media_call)call, FLAGS[f]), expected);
		}

	/* every single bit: only OBS_SOURCE_CONTROLLABLE_MEDIA admits */
	for (int bit = 0; bit < 32; bit++) {
		const uint32_t flags = 1u << bit;
		CHECK_INT(obs_source_media_admit_flags(OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE, flags),
			  bit == 13 ? READ_CALLBACK : REJECT);
		CHECK_INT(obs_source_media_admit_flags(OBS_SOURCE_MEDIA_CALL_STARTED, flags),
			  bit == 13 ? ACCEPT : REJECT);
	}
}

static void admit_flags_outside_enum(void **state)
{
	(void)state;

	/* a call outside the enum is treated as one with a callback */
	for (size_t c = 0; c < OUTSIDE_CALL_COUNT; c++) {
		const enum obs_source_media_call call = (enum obs_source_media_call)OUTSIDE_CALLS[c];
		CHECK_INT(obs_source_media_admit_flags(call, OBS_SOURCE_CONTROLLABLE_MEDIA), READ_CALLBACK);
		CHECK_INT(obs_source_media_admit_flags(call, UINT32_MAX), READ_CALLBACK);
		CHECK_INT(obs_source_media_admit_flags(call, 0), REJECT);
		CHECK_INT(obs_source_media_admit_flags(call, ~(uint32_t)OBS_SOURCE_CONTROLLABLE_MEDIA), REJECT);
	}
}

static void admit_callback(void **state)
{
	(void)state;

	CHECK_INT(obs_source_media_admit_callback(true), ACCEPT);
	CHECK_INT(obs_source_media_admit_callback(false), REJECT);
}

/* The steps in the callers' order (media_admit() and the callback check of
 * obs-source.c) for one call; counts the values read. */
static int admit_in_order(int call, bool valid, uint32_t flags, bool has_callback, int *flag_reads, int *callback_reads)
{
	int admit = (int)obs_source_media_admit(valid);
	if (admit == READ_FLAGS) {
		(*flag_reads)++;
		admit = (int)obs_source_media_admit_flags((enum obs_source_media_call)call, flags);
	}
	if (admit == READ_CALLBACK) {
		(*callback_reads)++;
		admit = (int)obs_source_media_admit_callback(has_callback);
	}
	return admit;
}

static void admission_every_input(void **state)
{
	(void)state;

	for (int call = 0; call < CALL_COUNT; call++)
		for (int valid = 0; valid < 2; valid++)
			for (size_t f = 0; f < FLAG_COUNT; f++)
				for (int cb = 0; cb < 2; cb++) {
					int flag_reads = 0, callback_reads = 0;
					const int admit =
						admit_in_order(call, valid, FLAGS[f], cb, &flag_reads, &callback_reads);
					const bool controllable = (FLAGS[f] & OBS_SOURCE_CONTROLLABLE_MEDIA) != 0;
					const bool accepted = valid && controllable && (!needs_callback(call) || cb);

					CHECK_INT(admit, accepted ? ACCEPT : REJECT);
					/* the flags only for a valid source, a callback only
					 * for a controllable one that has one to check */
					CHECK_INT(flag_reads, valid ? 1 : 0);
					CHECK_INT(callback_reads,
						  valid && controllable && needs_callback(call) ? 1 : 0);
				}

	/* characterized, not endorsed: _started() and _ended() emit their
	 * signal for any controllable source, with no callback to check (and
	 * obs-source.c checks them with obs_source_valid(), so a source with
	 * no data too) */
	int flag_reads = 0, callback_reads = 0;
	CHECK_INT(admit_in_order(OBS_SOURCE_MEDIA_CALL_STARTED, true, OBS_SOURCE_CONTROLLABLE_MEDIA, false, &flag_reads,
				 &callback_reads),
		  ACCEPT);
	CHECK_INT(admit_in_order(OBS_SOURCE_MEDIA_CALL_ENDED, true, OBS_SOURCE_CONTROLLABLE_MEDIA, false, &flag_reads,
				 &callback_reads),
		  ACCEPT);
	CHECK_INT(callback_reads, 0);
}

/* --- fallbacks ---------------------------------------------------------- */

static void getter_fallbacks(void **state)
{
	(void)state;

	/* obs_source_media_get_duration() and _get_time() */
	CHECK_INT(obs_source_media_time_fallback(), 0);
	/* obs_source_media_get_state() */
	CHECK_INT(obs_source_media_state_fallback(), OBS_MEDIA_STATE_NONE);
}

/* --- process_media_actions() ---------------------------------------------- */

static void dispatch_every_type(void **state)
{
	(void)state;

	CHECK_INT(obs_source_media_dispatch(OBS_SOURCE_MEDIA_ACTION_NONE), OBS_SOURCE_MEDIA_DISPATCH_RETURN);
	CHECK_INT(obs_source_media_dispatch(OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE), OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE);
	CHECK_INT(obs_source_media_dispatch(OBS_SOURCE_MEDIA_ACTION_RESTART), OBS_SOURCE_MEDIA_DISPATCH_RESTART);
	CHECK_INT(obs_source_media_dispatch(OBS_SOURCE_MEDIA_ACTION_STOP), OBS_SOURCE_MEDIA_DISPATCH_STOP);
	CHECK_INT(obs_source_media_dispatch(OBS_SOURCE_MEDIA_ACTION_NEXT), OBS_SOURCE_MEDIA_DISPATCH_NEXT);
	CHECK_INT(obs_source_media_dispatch(OBS_SOURCE_MEDIA_ACTION_PREVIOUS), OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS);
	CHECK_INT(obs_source_media_dispatch(OBS_SOURCE_MEDIA_ACTION_SET_TIME), OBS_SOURCE_MEDIA_DISPATCH_SET_TIME);
}

static void dispatch_outside_enum(void **state)
{
	(void)state;

	/* characterized, not endorsed: the switch has no case for it, so the
	 * action is dropped without a word and the next one is taken */
	for (size_t t = 0; t < OUTSIDE_TYPE_COUNT; t++)
		CHECK_INT(obs_source_media_dispatch(OUTSIDE_TYPES[t]), OBS_SOURCE_MEDIA_DISPATCH_SKIP);
}

static void signal_every_dispatch(void **state)
{
	(void)state;

	static const char *const SIGNALS[][2] = {
		/* by pause: false, true */
		[OBS_SOURCE_MEDIA_DISPATCH_RETURN] = {NULL, NULL},
		[OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE] = {"media_play", "media_pause"},
		[OBS_SOURCE_MEDIA_DISPATCH_RESTART] = {"media_restart", "media_restart"},
		[OBS_SOURCE_MEDIA_DISPATCH_STOP] = {"media_stopped", "media_stopped"},
		[OBS_SOURCE_MEDIA_DISPATCH_NEXT] = {"media_next", "media_next"},
		[OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS] = {"media_previous", "media_previous"},
		/* characterized, not endorsed: a set-time emits nothing */
		[OBS_SOURCE_MEDIA_DISPATCH_SET_TIME] = {NULL, NULL},
		[OBS_SOURCE_MEDIA_DISPATCH_SKIP] = {NULL, NULL},
	};

	for (int d = OBS_SOURCE_MEDIA_DISPATCH_RETURN; d <= OBS_SOURCE_MEDIA_DISPATCH_SKIP; d++) {
		const enum obs_source_media_dispatch dispatch = (enum obs_source_media_dispatch)d;
		CHECK_BOOL(obs_source_media_signal_reads_pause(dispatch), d == OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE);
		CHECK_STR(obs_source_media_signal(dispatch, false), SIGNALS[d][0]);
		CHECK_STR(obs_source_media_signal(dispatch, true), SIGNALS[d][1]);
	}
}

static void signal_outside_enum(void **state)
{
	(void)state;

	for (size_t d = 0; d < OUTSIDE_DISPATCH_COUNT; d++) {
		const enum obs_source_media_dispatch dispatch = (enum obs_source_media_dispatch)OUTSIDE_DISPATCHES[d];
		CHECK_BOOL(obs_source_media_signal_reads_pause(dispatch), false);
		CHECK_STR(obs_source_media_signal(dispatch, false), NULL);
		CHECK_STR(obs_source_media_signal(dispatch, true), NULL);
	}
}

/* process_media_actions() over a queue, written out with the moved
 * functions in its order: the callback, then the signal, until the queue
 * is empty. Each step goes into the trace. */
struct queued {
	int type;
	bool pause;
};

static void run_queue(const struct queued *queue, size_t count, char *trace, size_t size)
{
	static const char *const CALLBACKS[] = {
		[OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE] = "play_pause",
		[OBS_SOURCE_MEDIA_DISPATCH_RESTART] = "restart",
		[OBS_SOURCE_MEDIA_DISPATCH_STOP] = "stop",
		[OBS_SOURCE_MEDIA_DISPATCH_NEXT] = "next",
		[OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS] = "previous",
		[OBS_SOURCE_MEDIA_DISPATCH_SET_TIME] = "set_time",
	};
	size_t next = 0;

	trace[0] = 0;
	for (;;) {
		const int type = next < count ? queue[next].type : OBS_SOURCE_MEDIA_ACTION_NONE;
		const bool pause = next < count ? queue[next].pause : false;
		next++;

		const enum obs_source_media_dispatch dispatch = obs_source_media_dispatch(type);
		if (dispatch == OBS_SOURCE_MEDIA_DISPATCH_RETURN) {
			strncat(trace, "return", size - strlen(trace) - 1);
			return;
		}
		if (dispatch >= OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE &&
		    dispatch <= OBS_SOURCE_MEDIA_DISPATCH_SET_TIME) {
			strncat(trace, CALLBACKS[dispatch], size - strlen(trace) - 1);
			strncat(trace, " ", size - strlen(trace) - 1);
		}
		const char *signal =
			obs_source_media_signal(dispatch, obs_source_media_signal_reads_pause(dispatch) && pause);
		if (signal) {
			strncat(trace, signal, size - strlen(trace) - 1);
			strncat(trace, " ", size - strlen(trace) - 1);
		}
	}
}

static void process_every_action(void **state)
{
	(void)state;

	static const struct {
		struct queued action;
		const char *trace;
	} ONE[] = {
		{{OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE, false}, "play_pause media_play return"},
		{{OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE, true}, "play_pause media_pause return"},
		{{OBS_SOURCE_MEDIA_ACTION_RESTART, false}, "restart media_restart return"},
		{{OBS_SOURCE_MEDIA_ACTION_STOP, false}, "stop media_stopped return"},
		{{OBS_SOURCE_MEDIA_ACTION_NEXT, false}, "next media_next return"},
		{{OBS_SOURCE_MEDIA_ACTION_PREVIOUS, false}, "previous media_previous return"},
		{{OBS_SOURCE_MEDIA_ACTION_SET_TIME, false}, "set_time return"},
		/* the pause is not read for any other action */
		{{OBS_SOURCE_MEDIA_ACTION_RESTART, true}, "restart media_restart return"},
		{{OBS_SOURCE_MEDIA_ACTION_SET_TIME, true}, "set_time return"},
		/* characterized, not endorsed: skipped, the loop goes on */
		{{7, true}, "return"},
		{{-1, false}, "return"},
		{{INT_MAX, false}, "return"},
		/* a queued MEDIA_ACTION_NONE ends the loop like an empty queue */
		{{OBS_SOURCE_MEDIA_ACTION_NONE, false}, "return"},
	};
	char trace[256];

	for (size_t i = 0; i < sizeof(ONE) / sizeof(ONE[0]); i++) {
		run_queue(&ONE[i].action, 1, trace, sizeof(trace));
		CHECK_STR(trace, ONE[i].trace);
	}

	run_queue(NULL, 0, trace, sizeof(trace));
	CHECK_STR(trace, "return");

	/* in queue order; an unknown type in the middle is skipped, a queued
	 * MEDIA_ACTION_NONE stops the loop with actions still queued */
	const struct queued mixed[] = {
		{OBS_SOURCE_MEDIA_ACTION_STOP, false},      {99, true},
		{OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE, true}, {OBS_SOURCE_MEDIA_ACTION_SET_TIME, false},
		{OBS_SOURCE_MEDIA_ACTION_NONE, false},      {OBS_SOURCE_MEDIA_ACTION_NEXT, false},
	};
	run_queue(mixed, sizeof(mixed) / sizeof(mixed[0]), trace, sizeof(trace));
	CHECK_STR(trace, "stop media_stopped play_pause media_pause set_time return");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enum_values),
		cmocka_unit_test(admit_valid),
		cmocka_unit_test(admit_flags_every_call),
		cmocka_unit_test(admit_flags_outside_enum),
		cmocka_unit_test(admit_callback),
		cmocka_unit_test(admission_every_input),
		cmocka_unit_test(getter_fallbacks),
		cmocka_unit_test(dispatch_every_type),
		cmocka_unit_test(dispatch_outside_enum),
		cmocka_unit_test(signal_every_dispatch),
		cmocka_unit_test(signal_outside_enum),
		cmocka_unit_test(process_every_action),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
