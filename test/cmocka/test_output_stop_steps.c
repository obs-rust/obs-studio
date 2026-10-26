#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <errno.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-stop.h"
#include "obs-output-raw-audio.h"
#include "obs-output-video-checks.h"

/*
 * Characterization of the output stop and end-capture decisions of
 * libobs/obs-output.c, moved to libobs/obs-output-stop.c before their Rust
 * port:
 * - obs_output_stop(): the delayed stop, an encoded output
 *   (obs_output_stop_encoded) with a delay running (_delay_on);
 * - end_data_capture_thread(): which encoders or raw connections stop
 *   (_has_video, _has_audio, _encoded), the process_delay() callback and the
 *   delay cleanup (_delay_on), and the service deactivation (_service);
 * - obs_output_end_data_capture_internal(): the unstarted encoders' shutdown
 *   (_encoded), whether the delay ends or its restart references keep it
 *   (_end_delay), the frame log (_has_video) and the thread creation
 *   fallback (_thread_failed).
 * end_data_capture_thread() asks obs_output_raw_audio_interleave() for the
 * callback of an undelayed encoded output, and stop_raw_audio() asks
 * obs_output_raw_audio_mix_on() for each mix with raw_audio2, else
 * obs_output_first_mixer(); those are pinned here at those callers' values.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The bodies of obs_output_stop(), obs_output_force_stop(),
 * obs_output_actual_stop(), stop_raw_audio(), end_data_capture_thread(),
 * shutdown_unstarted_encoders_thread(), obs_output_end_data_capture_internal(),
 * obs_output_end_data_capture() and obs_output_signal_stop() are also copied
 * below onto a stand-in output (no thread, no lock, no service and no
 * encoder: their results are given, a created thread runs only when the test
 * says so, and every call, event, signal and warning is recorded), so every
 * route over the atomics is pinned end to end: each of active, data_active,
 * reconnecting, delay_active, delay_capturing and the end-capture thread's
 * flag set and clear, the stopping event set and clear, a forced stop of a
 * delayed output that is and is not capturing, a reconnecting output stopped
 * with and without data, the end with and without its signal, restart refs
 * 0, 1 and -1, a thread that cannot be created (the inline fallback), and
 * each callback.
 *
 * The functions are hidden in libobs, so this test compiles the C sources
 * into itself and calls only them, through their headers. The real-libobs
 * tests test_output_end_capture_thread_join, test_output_deactivate_state,
 * test_null_output_stop_thread, test_ffmpeg_output_stop,
 * test_ffmpeg_output_drain, test_encoder_error_stop,
 * test_replay_buffer_pause_stop, test_output_delay_e2e,
 * test_output_failed_start_encoders, test_recording_shutdown and
 * test_pthread_init_failures cover the callers.
 *
 * Nothing here comes from outside OBS: the flags come from the output's
 * plugin, the delay and the mixers from the frontend.
 */

#define F_VIDEO 1u
#define F_AUDIO 2u
#define F_ENCODED 4u
#define F_SERVICE 8u

#define STREAM (F_VIDEO | F_AUDIO | F_ENCODED | F_SERVICE)
#define RECORDING (F_VIDEO | F_AUDIO | F_ENCODED)
#define RAW_VIDEO F_VIDEO
#define RAW_AV (F_VIDEO | F_AUDIO)

/* OBS_OUTPUT_SUCCESS, OBS_OUTPUT_DISCONNECTED, OBS_OUTPUT_ERROR */
#define CODE_SUCCESS 0
#define CODE_DISCONNECTED -2
#define CODE_ERROR -4

static const long edge_long[] = {LONG_MIN, -2, -1, 0, 1, 2, 3, LONG_MAX};
#define N_EDGE_LONG (sizeof(edge_long) / sizeof(edge_long[0]))
static const uint64_t edge_u64[] = {0, 1, 2, 1000000000ULL, 20000000000ULL, 0x7fffffffffffffffULL, UINT64_MAX};
#define N_EDGE_U64 (sizeof(edge_u64) / sizeof(edge_u64[0]))
static const int edge_int[] = {INT_MIN, -1, 0, 1, EAGAIN, EINVAL, INT_MAX};
#define N_EDGE_INT (sizeof(edge_int) / sizeof(edge_int[0]))

/* ------------------------------------------------------------------------ */
/* the constants and the rules */

static void constants(void **state)
{
	(void)state;
	/* OBS_OUTPUT_VIDEO, _AUDIO, _ENCODED, _SERVICE */
	assert_int_equal(OBS_OUTPUT_STOP_VIDEO, 1);
	assert_int_equal(OBS_OUTPUT_STOP_AUDIO, 2);
	assert_int_equal(OBS_OUTPUT_STOP_ENCODED, 4);
	assert_int_equal(OBS_OUTPUT_STOP_SERVICE, 8);
}

static void flag_rules(void **state)
{
	(void)state;
	/* every low 16-bit flag word, with and without high bits: each rule
	 * looks at its own bit only */
	static const uint32_t high[] = {0, 0x10000u, 0x80000000u, 0xffff0000u};
	for (size_t h = 0; h < sizeof(high) / sizeof(high[0]); h++) {
		for (uint32_t low = 0; low <= 0xffffu; low++) {
			const uint32_t f = high[h] | low;
			assert_int_equal(obs_output_stop_encoded(f), (f & F_ENCODED) != 0);
			assert_int_equal(obs_output_stop_has_video(f), (f & F_VIDEO) != 0);
			assert_int_equal(obs_output_stop_has_audio(f), (f & F_AUDIO) != 0);
			assert_int_equal(obs_output_stop_service(f), (f & F_SERVICE) != 0);
		}
	}
	/* the common outputs: an encoded stream (AV, encoded, service), a
	 * recording (AV, encoded), a raw virtual camera (video only) */
	assert_true(obs_output_stop_encoded(STREAM));
	assert_true(obs_output_stop_service(STREAM));
	assert_true(obs_output_stop_encoded(RECORDING));
	assert_false(obs_output_stop_service(RECORDING));
	assert_false(obs_output_stop_encoded(RAW_VIDEO));
	assert_true(obs_output_stop_has_video(RAW_VIDEO));
	assert_false(obs_output_stop_has_audio(RAW_VIDEO));
	assert_false(obs_output_stop_has_video(0));
	assert_false(obs_output_stop_has_audio(0));
}

static void delay_on(void **state)
{
	(void)state;
	/* a delay runs while active_delay_ns is not 0, however short */
	assert_false(obs_output_stop_delay_on(0));
	assert_true(obs_output_stop_delay_on(1));
	for (size_t i = 0; i < N_EDGE_U64; i++)
		assert_int_equal(obs_output_stop_delay_on(edge_u64[i]), edge_u64[i] != 0);
}

static void end_delay(void **state)
{
	(void)state;
	/* only no restart reference ends the delay */
	assert_true(obs_output_stop_end_delay(0));
	assert_false(obs_output_stop_end_delay(1));
	assert_false(obs_output_stop_end_delay(2));
	/* a negative count keeps the delay too: the test is "0", not "not
	 * positive" (characterized, not endorsed; the count is only incremented
	 * on a reconnect and decremented on a start after a "not 0" test, so it
	 * never goes below 0) */
	assert_false(obs_output_stop_end_delay(-1));
	for (size_t i = 0; i < N_EDGE_LONG; i++)
		assert_int_equal(obs_output_stop_end_delay(edge_long[i]), edge_long[i] == 0);
}

static void thread_failed(void **state)
{
	(void)state;
	/* pthread_create() returns 0 or an error number; any other value fails
	 * too */
	assert_false(obs_output_stop_thread_failed(0));
	assert_true(obs_output_stop_thread_failed(EAGAIN));
	for (size_t i = 0; i < N_EDGE_INT; i++)
		assert_int_equal(obs_output_stop_thread_failed(edge_int[i]), edge_int[i] != 0);
}

static void shared_rules(void **state)
{
	(void)state;
	/* end_data_capture_thread(): an undelayed encoded output stops its
	 * encoders with interleave_packets() only with both video and audio */
	assert_true(obs_output_raw_audio_interleave(true, true));
	assert_false(obs_output_raw_audio_interleave(true, false));
	assert_false(obs_output_raw_audio_interleave(false, true));
	assert_false(obs_output_raw_audio_interleave(false, false));

	/* stop_raw_audio(): each mix whose bit is set, with raw_audio2 */
	for (size_t mask = 0; mask < 64; mask++)
		for (size_t idx = 0; idx < 6; idx++)
			assert_int_equal(obs_output_raw_audio_mix_on(mask, idx), (mask & ((size_t)1 << idx)) != 0);
	/* bits past the mixes are never asked (MAX_AUDIO_MIXES is 6) */
	assert_false(obs_output_raw_audio_mix_on((size_t)1 << 6, 0));

	/* without raw_audio2: the lowest mix whose bit is set, 0 for none */
	assert_int_equal(obs_output_first_mixer(0), 0);
	assert_int_equal(obs_output_first_mixer(1), 0);
	assert_int_equal(obs_output_first_mixer(6), 1);
	assert_int_equal(obs_output_first_mixer(0x20), 5);
	assert_int_equal(obs_output_first_mixer(0x40), 0);
}

/* ------------------------------------------------------------------------ */
/* the callers, copied onto a stand-in output */

#define SLOTS 2

enum pending_thread {
	PENDING_NONE,
	PENDING_END,
	PENDING_UNSTARTED,
};

struct stand_track {
	bool present;
	int captions;
};

struct stand_output {
	/* the output */
	bool has_data;
	uint32_t flags;
	bool raw_audio2;
	size_t mixer_mask;
	uint64_t active_delay_ns;
	bool video_enc[SLOTS], audio_enc[SLOTS];
	struct stand_track tracks[SLOTS];
	size_t keyframes;
	int stop_code;
	/* the atomics and events */
	bool active, data_active, reconnecting, delay_active, delay_capturing, starting;
	bool thread_active, thread_joinable;
	long delay_restart_refs;
	bool stopping_set, reconnect_stop_set;
	bool reconnect_thread_active;
	/* the results of the calls the C keeps */
	bool valid;
	int create_result;
	bool can_reconnect;
	/* a created thread that has not run yet */
	enum pending_thread pending;
	/* the record */
	char log[2048];
};

static void rec(struct stand_output *o, const char *fmt, ...)
{
	const size_t n = strlen(o->log);
	va_list ap;
	if (n)
		snprintf(o->log + n, sizeof(o->log) - n, " ");
	va_start(ap, fmt);
	vsnprintf(o->log + strlen(o->log), sizeof(o->log) - strlen(o->log), fmt, ap);
	va_end(ap);
}

static void stand_init(struct stand_output *o, uint32_t flags)
{
	memset(o, 0, sizeof(*o));
	o->has_data = true;
	o->flags = flags;
	o->video_enc[0] = true;
	o->audio_enc[0] = true;
	o->tracks[0].present = true;
	o->tracks[0].captions = 2;
	o->keyframes = 3;
	o->stop_code = CODE_ERROR;
	o->valid = true;
	/* a running output: active, its data active, the stopping event set
	 * (it is reset only while a stop is under way) */
	o->active = true;
	o->data_active = true;
	o->stopping_set = true;
	o->starting = true;
}

static void clear_log(struct stand_output *o)
{
	o->log[0] = 0;
}

/* stopping(): the stopping event is not set */
static bool stand_stopping(const struct stand_output *o)
{
	return !o->stopping_set;
}

static void stand_end_data_capture(struct stand_output *o);

/* obs_output_actual_stop() */
static void stand_actual_stop(struct stand_output *o, bool force, uint64_t ts)
{
	bool call_stop = true;
	bool was_reconnecting = false;

	if (stand_stopping(o) && !force)
		return;

	rec(o, "unpause");

	o->stopping_set = false;
	rec(o, "reset-stopping");

	was_reconnecting = o->reconnecting && !o->delay_active;
	if (o->reconnecting) {
		o->reconnect_stop_set = true;
		rec(o, "set-reconnect-stop");
		if (o->reconnect_thread_active)
			rec(o, "join-reconnect");
	}

	if (force) {
		if (o->delay_active) {
			call_stop = o->delay_capturing;
			o->delay_active = false;
			o->delay_capturing = false;
			o->stop_code = CODE_SUCCESS;
			stand_end_data_capture(o);
			o->stopping_set = true;
			rec(o, "set-stopping");
		} else {
			call_stop = true;
		}
	} else {
		call_stop = true;
	}

	if (o->has_data && call_stop) {
		rec(o, "info.stop-%llu", (unsigned long long)ts);

	} else if (was_reconnecting) {
		o->stop_code = CODE_SUCCESS;
		rec(o, "signal-stop-%d", o->stop_code);
		o->stopping_set = true;
		rec(o, "set-stopping");
	}

	o->starting = false;

	for (size_t i = 0; i < SLOTS; i++) {
		struct stand_track *t = &o->tracks[i];
		if (!t->present) {
			continue;
		}
		rec(o, "free-captions-%zu-%d", i, t->captions);
		t->captions = 0;
	}

	o->keyframes = 0;
}

static void stand_force_stop(struct stand_output *o);

/* obs_output_stop() */
static void stand_stop(struct stand_output *o)
{
	if (!o->valid)
		return;
	if (!o->has_data)
		return;
	if (!o->active && !o->reconnecting)
		return;
	if (o->reconnecting) {
		stand_force_stop(o);
		return;
	}

	if (obs_output_stop_encoded(o->flags) && obs_output_stop_delay_on(o->active_delay_ns)) {
		rec(o, "delay-stop");
	} else if (!stand_stopping(o)) {
		rec(o, "signal-stopping");
		stand_actual_stop(o, false, 7);
	}
}

/* obs_output_force_stop() */
static void stand_force_stop(struct stand_output *o)
{
	if (!o->valid)
		return;

	if (!stand_stopping(o)) {
		o->stop_code = 0;
		rec(o, "signal-stopping");
	}
	stand_actual_stop(o, true, 0);
}

static const char *callback_name(const struct stand_output *o, bool has_video, bool has_audio)
{
	if (obs_output_stop_delay_on(o->active_delay_ns))
		return "delay";
	else
		return obs_output_raw_audio_interleave(has_video, has_audio) ? "interleave" : "default";
}

/* stop_raw_audio() */
static void stand_stop_raw_audio(struct stand_output *o)
{
	if (o->raw_audio2) {
		for (int idx = 0; idx < 6; idx++) {
			if (obs_output_raw_audio_mix_on(o->mixer_mask, (size_t)idx)) {
				rec(o, "disconnect-mix-%d", idx);
			}
		}
	} else {
		rec(o, "disconnect-mix-%zu", obs_output_first_mixer(o->mixer_mask));
	}
}

/* end_data_capture_thread() */
static void stand_end_data_capture_thread(struct stand_output *o)
{
	const char *encoded_callback;
	bool has_video = obs_output_stop_has_video(o->flags);
	bool has_audio = obs_output_stop_has_audio(o->flags);

	if (obs_output_stop_encoded(o->flags)) {
		encoded_callback = callback_name(o, has_video, has_audio);

		if (has_video)
			for (size_t i = 0; i < SLOTS; i++)
				if (o->video_enc[i])
					rec(o, "stop-video-%zu-%s", i, encoded_callback);
		if (has_audio)
			for (size_t i = 0; i < SLOTS; i++)
				if (o->audio_enc[i])
					rec(o, "stop-audio-%zu-%s", i, encoded_callback);
	} else {
		if (has_video)
			rec(o, "stop-raw-video");
		if (has_audio)
			stand_stop_raw_audio(o);
	}

	if (obs_output_stop_service(o->flags))
		rec(o, "deactivate-service");

	if (obs_output_stop_delay_on(o->active_delay_ns))
		rec(o, "cleanup-delay");

	o->active = false;
	rec(o, "signal-deactivate");
	o->stopping_set = true;
	rec(o, "set-stopping");
	o->thread_active = false;
}

/* shutdown_unstarted_encoders_thread() */
static void stand_shutdown_unstarted_thread(struct stand_output *o)
{
	for (size_t i = 0; i < SLOTS; i++)
		if (o->video_enc[i])
			rec(o, "shutdown-video-%zu", i);
	for (size_t i = 0; i < SLOTS; i++)
		if (o->audio_enc[i])
			rec(o, "shutdown-audio-%zu", i);
	o->thread_active = false;
}

/* the created thread runs */
static void stand_run_thread(struct stand_output *o)
{
	enum pending_thread p = o->pending;
	o->pending = PENDING_NONE;
	if (p == PENDING_END)
		stand_end_data_capture_thread(o);
	else if (p == PENDING_UNSTARTED)
		stand_shutdown_unstarted_thread(o);
}

/* join_end_data_capture_thread() */
static void stand_join_end_thread(struct stand_output *o)
{
	bool joinable = o->thread_joinable;
	o->thread_joinable = false;
	if (joinable)
		rec(o, "join-end-thread");
}

/* obs_output_end_data_capture_internal() */
static void stand_end_data_capture_internal(struct stand_output *o, bool signal)
{
	int ret;

	if (!o->valid)
		return;

	o->starting = false;

	if (!o->active || !o->data_active) {
		if (signal) {
			int params_code = o->stop_code;
			rec(o, "stop-params");
			if (!o->active && obs_output_stop_encoded(o->flags) && !o->thread_active) {
				stand_join_end_thread(o);
				o->thread_active = true;
				ret = o->create_result;
				rec(o, "create-unstarted-thread");
				if (obs_output_stop_thread_failed(ret)) {
					o->thread_active = false;
				} else {
					o->thread_joinable = true;
					o->pending = PENDING_UNSTARTED;
				}
			}
			rec(o, "signal-stop-%d", params_code);
			o->stop_code = CODE_SUCCESS;
			o->stopping_set = true;
			rec(o, "set-stopping");
		}
		return;
	}

	if (o->delay_active) {
		o->delay_capturing = false;

		if (obs_output_stop_end_delay(o->delay_restart_refs)) {
			o->delay_active = false;
		} else {
			o->stopping_set = true;
			rec(o, "set-stopping");
			return;
		}
	}

	o->data_active = false;

	if (obs_output_stop_has_video(o->flags))
		rec(o, "log-frames");

	stand_join_end_thread(o);

	int params_code = 0;
	if (signal) {
		params_code = o->stop_code;
		rec(o, "stop-params");
	}

	o->thread_active = true;
	ret = o->create_result;
	rec(o, "create-end-thread");
	if (obs_output_stop_thread_failed(ret)) {
		rec(o, "warn-thread");
		stand_end_data_capture_thread(o);
	} else {
		o->thread_joinable = true;
		o->pending = PENDING_END;
	}

	if (signal) {
		rec(o, "signal-stop-%d", params_code);
		o->stop_code = CODE_SUCCESS;
	}
}

/* obs_output_end_data_capture() */
static void stand_end_data_capture(struct stand_output *o)
{
	stand_end_data_capture_internal(o, true);
}

/* obs_output_signal_stop() */
static void stand_signal_stop(struct stand_output *o, int code)
{
	if (!o->valid)
		return;

	o->stop_code = code;

	if (o->can_reconnect) {
		if (o->delay_active)
			o->delay_restart_refs++;
		stand_end_data_capture_internal(o, false);
		rec(o, "reconnect");
	} else {
		if (o->delay_active) {
			o->delay_capturing = false;
			o->delay_active = false;
		}
		if (o->reconnecting)
			o->reconnecting = false;
		stand_end_data_capture(o);
	}
}

/* ------------------------------------------------------------------------ */
/* obs_output_stop() and obs_output_force_stop() */

static void stop_routes(void **state)
{
	(void)state;
	struct stand_output o;

	/* a stream: "stopping", then the plugin's stop */
	stand_init(&o, STREAM);
	stand_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2");
	assert_false(o.stopping_set);
	assert_false(o.starting);
	assert_int_equal(o.keyframes, 0);
	assert_int_equal(o.tracks[0].captions, 0);
	/* the stop code is left for the plugin's stop to set */
	assert_int_equal(o.stop_code, CODE_ERROR);
	/* still active: only the end of the capture deactivates it */
	assert_true(o.active);

	/* a raw output stops the same way */
	stand_init(&o, RAW_VIDEO);
	stand_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2");

	/* invalid, no data, or neither active nor reconnecting: nothing */
	stand_init(&o, STREAM);
	o.valid = false;
	stand_stop(&o);
	assert_string_equal(o.log, "");
	stand_init(&o, STREAM);
	o.has_data = false;
	stand_stop(&o);
	assert_string_equal(o.log, "");
	stand_init(&o, STREAM);
	o.active = false;
	stand_stop(&o);
	assert_string_equal(o.log, "");
	assert_true(o.starting);

	/* already stopping: nothing, not even "stopping" */
	stand_init(&o, STREAM);
	o.stopping_set = false;
	stand_stop(&o);
	assert_string_equal(o.log, "");

	/* data_active, delay_active and delay_capturing do not change the route
	 * (characterized, not endorsed: a delay without active_delay_ns stops
	 * at once) */
	stand_init(&o, RECORDING);
	o.data_active = false;
	o.delay_active = true;
	o.delay_capturing = true;
	stand_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2");
	assert_true(o.delay_active);
}

static void stop_delayed(void **state)
{
	(void)state;
	struct stand_output o;

	/* an encoded output with a delay running stops through the delay, even
	 * while stopping */
	stand_init(&o, STREAM);
	o.active_delay_ns = 20000000000ULL;
	stand_stop(&o);
	assert_string_equal(o.log, "delay-stop");
	assert_true(o.starting);
	stand_init(&o, RECORDING);
	o.active_delay_ns = 1;
	o.stopping_set = false;
	stand_stop(&o);
	assert_string_equal(o.log, "delay-stop");

	/* a raw output with active_delay_ns stops at once (characterized, not
	 * endorsed: only an encoded output sets it) */
	stand_init(&o, RAW_AV);
	o.active_delay_ns = 1;
	stand_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2");

	/* reconnecting comes first: a delayed output that is reconnecting is
	 * force stopped */
	stand_init(&o, STREAM);
	o.active_delay_ns = 1;
	o.reconnecting = true;
	stand_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping set-reconnect-stop info.stop-0 "
				   "free-captions-0-2");
}

static void stop_reconnecting(void **state)
{
	(void)state;
	struct stand_output o;

	/* reconnecting with data: a forced stop, the plugin's stop runs */
	stand_init(&o, STREAM);
	o.active = false;
	o.reconnecting = true;
	o.reconnect_thread_active = true;
	stand_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping set-reconnect-stop join-reconnect "
				   "info.stop-0 free-captions-0-2");
	assert_int_equal(o.stop_code, CODE_SUCCESS);
	assert_true(o.reconnect_stop_set);
	/* the reconnect thread clears reconnecting itself */
	assert_true(o.reconnecting);

	/* reconnecting, the thread already gone: no join */
	stand_init(&o, STREAM);
	o.reconnecting = true;
	stand_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping set-reconnect-stop info.stop-0 "
				   "free-captions-0-2");

	/* reconnecting without data: obs_output_stop() refuses before */
	stand_init(&o, STREAM);
	o.reconnecting = true;
	o.has_data = false;
	stand_stop(&o);
	assert_string_equal(o.log, "");

	/* forced without data while reconnecting: the stop is signalled here */
	stand_init(&o, STREAM);
	o.reconnecting = true;
	o.has_data = false;
	stand_force_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping set-reconnect-stop signal-stop-0 "
				   "set-stopping free-captions-0-2");
	assert_true(o.stopping_set);

	/* reconnecting with a delay active: not "was reconnecting"; the forced
	 * stop clears the delay before it ends the capture, so the end takes the
	 * full route whatever the restart refs */
	stand_init(&o, STREAM);
	o.reconnecting = true;
	o.has_data = false;
	o.delay_active = true;
	o.delay_restart_refs = 1;
	stand_force_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping set-reconnect-stop log-frames stop-params "
				   "create-end-thread signal-stop-0 set-stopping free-captions-0-2");
	assert_false(o.delay_active);
	assert_int_equal(o.delay_restart_refs, 1);
}

static void force_stop(void **state)
{
	(void)state;
	struct stand_output o;

	/* not stopping: the code is cleared and "stopping" signalled */
	stand_init(&o, STREAM);
	stand_force_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping info.stop-0 free-captions-0-2");
	assert_int_equal(o.stop_code, CODE_SUCCESS);

	/* stopping: no signal and the code kept, the stop goes on */
	stand_init(&o, STREAM);
	o.stopping_set = false;
	stand_force_stop(&o);
	assert_string_equal(o.log, "unpause reset-stopping info.stop-0 free-captions-0-2");
	assert_int_equal(o.stop_code, CODE_ERROR);

	/* not active: a forced stop still calls the plugin's stop
	 * (characterized, not endorsed) */
	stand_init(&o, STREAM);
	o.active = false;
	o.data_active = false;
	stand_force_stop(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping info.stop-0 free-captions-0-2");

	/* invalid: nothing */
	stand_init(&o, STREAM);
	o.valid = false;
	stand_force_stop(&o);
	assert_string_equal(o.log, "");
}

/* ------------------------------------------------------------------------ */
/* obs_output_actual_stop() */

static void actual_stop_refusal(void **state)
{
	(void)state;
	struct stand_output o;

	/* not forced while stopping: refused, nothing cleared */
	stand_init(&o, STREAM);
	o.stopping_set = false;
	stand_actual_stop(&o, false, 5);
	assert_string_equal(o.log, "");
	assert_true(o.starting);
	assert_int_equal(o.keyframes, 3);

	/* forced while stopping: goes on */
	stand_init(&o, STREAM);
	o.stopping_set = false;
	stand_actual_stop(&o, true, 5);
	assert_string_equal(o.log, "unpause reset-stopping info.stop-5 free-captions-0-2");

	/* each caption track present is freed, the others skipped */
	stand_init(&o, STREAM);
	o.tracks[0].present = false;
	o.tracks[1].present = true;
	o.tracks[1].captions = 0;
	stand_actual_stop(&o, false, 5);
	assert_string_equal(o.log, "unpause reset-stopping info.stop-5 free-captions-1-0");
}

static void actual_stop_forced_delay(void **state)
{
	(void)state;
	struct stand_output o;

	/* a delayed output that is capturing: the capture ends (the end-capture
	 * thread is created), then the plugin's stop */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.delay_capturing = true;
	stand_actual_stop(&o, true, 0);
	assert_string_equal(o.log, "unpause reset-stopping log-frames stop-params create-end-thread signal-stop-0 "
				   "set-stopping info.stop-0 free-captions-0-2");
	assert_false(o.delay_active);
	assert_false(o.delay_capturing);
	assert_int_equal(o.pending, PENDING_END);

	/* not capturing yet: the plugin's stop is not called */
	stand_init(&o, STREAM);
	o.delay_active = true;
	stand_actual_stop(&o, true, 0);
	assert_string_equal(o.log, "unpause reset-stopping log-frames stop-params create-end-thread signal-stop-0 "
				   "set-stopping free-captions-0-2");

	/* not forced: the delay is left alone */
	stand_init(&o, STREAM);
	o.delay_active = true;
	stand_actual_stop(&o, false, 9);
	assert_string_equal(o.log, "unpause reset-stopping info.stop-9 free-captions-0-2");
	assert_true(o.delay_active);

	/* forced, delayed, data not active: the end signals the stop and shuts
	 * nothing down (still active) */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.data_active = false;
	stand_actual_stop(&o, true, 0);
	assert_string_equal(o.log, "unpause reset-stopping stop-params signal-stop-0 set-stopping set-stopping "
				   "free-captions-0-2");
}

/* ------------------------------------------------------------------------ */
/* obs_output_end_data_capture_internal() */

static void end_not_active(void **state)
{
	(void)state;
	struct stand_output o;

	/* an encoded output stopped before its capture began: the stop is
	 * signalled with the code copied first, and the encoders it initialized
	 * are shut down on a thread */
	stand_init(&o, STREAM);
	o.active = false;
	o.data_active = false;
	o.thread_joinable = true;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "stop-params join-end-thread create-unstarted-thread signal-stop--4 set-stopping");
	assert_int_equal(o.stop_code, CODE_SUCCESS);
	assert_false(o.starting);
	assert_true(o.thread_active);
	assert_true(o.thread_joinable);
	stand_run_thread(&o);
	assert_string_equal(o.log, "stop-params join-end-thread create-unstarted-thread signal-stop--4 "
				   "set-stopping shutdown-video-0 shutdown-audio-0");
	assert_false(o.thread_active);

	/* no end signal (a reconnect): only starting is cleared */
	stand_init(&o, STREAM);
	o.active = false;
	stand_end_data_capture_internal(&o, false);
	assert_string_equal(o.log, "");
	assert_false(o.starting);
	assert_int_equal(o.stop_code, CODE_ERROR);

	/* a raw output: no thread */
	stand_init(&o, RAW_AV);
	o.active = false;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "stop-params signal-stop--4 set-stopping");

	/* an end-capture thread still running: no second one */
	stand_init(&o, RECORDING);
	o.active = false;
	o.thread_active = true;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "stop-params signal-stop--4 set-stopping");

	/* active but its data not active (a stream still connecting): signalled,
	 * no thread (its encoders belong to the capture that never began) */
	stand_init(&o, STREAM);
	o.data_active = false;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "stop-params signal-stop--4 set-stopping");

	/* the thread cannot be created: the flag is cleared, the stop signalled */
	stand_init(&o, STREAM);
	o.active = false;
	o.create_result = EAGAIN;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "stop-params create-unstarted-thread signal-stop--4 set-stopping");
	assert_false(o.thread_active);
	assert_false(o.thread_joinable);
	assert_int_equal(o.pending, PENDING_NONE);

	/* invalid: nothing, starting kept */
	stand_init(&o, STREAM);
	o.valid = false;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "");
	assert_true(o.starting);
}

static void end_delay_route(void **state)
{
	(void)state;
	struct stand_output o;

	/* no restart reference: the delay ends with the capture */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.delay_capturing = true;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "log-frames stop-params create-end-thread signal-stop--4");
	assert_false(o.delay_active);
	assert_false(o.delay_capturing);
	assert_false(o.data_active);

	/* a restart reference keeps the delay: the stopping event is set and the
	 * capture goes on */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.delay_capturing = true;
	o.delay_restart_refs = 1;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "set-stopping");
	assert_true(o.delay_active);
	assert_false(o.delay_capturing);
	assert_true(o.data_active);
	assert_int_equal(o.stop_code, CODE_ERROR);
	assert_int_equal(o.delay_restart_refs, 1);

	/* a negative count keeps it too (characterized, not endorsed) */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.delay_restart_refs = -1;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "set-stopping");
	assert_true(o.delay_active);
}

static void end_full(void **state)
{
	(void)state;
	struct stand_output o;

	/* a stream: the frame log, the last thread joined, the code copied
	 * before the thread starts, the stop signalled after */
	stand_init(&o, STREAM);
	o.thread_joinable = true;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "log-frames join-end-thread stop-params create-end-thread signal-stop--4");
	assert_int_equal(o.stop_code, CODE_SUCCESS);
	assert_true(o.active);
	assert_true(o.thread_active);
	assert_true(o.thread_joinable);
	stand_run_thread(&o);
	assert_string_equal(o.log, "log-frames join-end-thread stop-params create-end-thread signal-stop--4 "
				   "stop-video-0-interleave stop-audio-0-interleave deactivate-service "
				   "signal-deactivate set-stopping");
	assert_false(o.active);
	assert_false(o.thread_active);
	assert_true(o.stopping_set);

	/* no end signal: no code copied, none signalled, the code kept */
	stand_init(&o, STREAM);
	stand_end_data_capture_internal(&o, false);
	assert_string_equal(o.log, "log-frames create-end-thread");
	assert_int_equal(o.stop_code, CODE_ERROR);

	/* an audio-only output: no frame log */
	stand_init(&o, F_AUDIO | F_ENCODED);
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "stop-params create-end-thread signal-stop--4");

	/* the thread cannot be created: a warning and the thread's work inline,
	 * before the stop is signalled */
	stand_init(&o, RECORDING);
	o.create_result = EAGAIN;
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "log-frames stop-params create-end-thread warn-thread stop-video-0-interleave "
				   "stop-audio-0-interleave signal-deactivate set-stopping signal-stop--4");
	assert_false(o.active);
	assert_false(o.thread_active);
	assert_false(o.thread_joinable);
	assert_int_equal(o.pending, PENDING_NONE);
}

/* ------------------------------------------------------------------------ */
/* end_data_capture_thread() */

static void end_thread_callbacks(void **state)
{
	(void)state;
	struct stand_output o;

	/* a delay running: process_delay(), and the delay cleaned up */
	stand_init(&o, STREAM);
	o.active_delay_ns = 1;
	o.video_enc[1] = true;
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "stop-video-0-delay stop-video-1-delay stop-audio-0-delay deactivate-service "
				   "cleanup-delay signal-deactivate set-stopping");

	/* video only and audio only: the default callback */
	stand_init(&o, F_VIDEO | F_ENCODED);
	o.audio_enc[1] = true;
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "stop-video-0-default signal-deactivate set-stopping");
	stand_init(&o, F_AUDIO | F_ENCODED);
	o.audio_enc[1] = true;
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "stop-audio-0-default stop-audio-1-default signal-deactivate set-stopping");

	/* an encoded output with neither flag stops nothing (characterized, not
	 * endorsed) */
	stand_init(&o, F_ENCODED | F_SERVICE);
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "deactivate-service signal-deactivate set-stopping");

	/* empty slots are skipped */
	stand_init(&o, RECORDING);
	o.video_enc[0] = false;
	o.audio_enc[0] = false;
	o.audio_enc[1] = true;
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "stop-audio-1-interleave signal-deactivate set-stopping");
}

static void end_thread_raw(void **state)
{
	(void)state;
	struct stand_output o;

	/* raw video */
	stand_init(&o, RAW_VIDEO);
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "stop-raw-video signal-deactivate set-stopping");

	/* raw audio with raw_audio2: each mix in the mask */
	stand_init(&o, RAW_AV);
	o.raw_audio2 = true;
	o.mixer_mask = 0x25;
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "stop-raw-video disconnect-mix-0 disconnect-mix-2 disconnect-mix-5 "
				   "signal-deactivate set-stopping");
	/* bits past the mixes are never asked */
	stand_init(&o, F_AUDIO);
	o.raw_audio2 = true;
	o.mixer_mask = 0xc0;
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "signal-deactivate set-stopping");

	/* without raw_audio2: the first mixer only, 0 for an empty mask */
	stand_init(&o, F_AUDIO);
	o.mixer_mask = 0x06;
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "disconnect-mix-1 signal-deactivate set-stopping");
	stand_init(&o, F_AUDIO);
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "disconnect-mix-0 signal-deactivate set-stopping");

	/* a raw output with the service flag deactivates it, and one with
	 * active_delay_ns cleans the delay up (characterized, not endorsed) */
	stand_init(&o, RAW_VIDEO | F_SERVICE);
	o.active_delay_ns = 5;
	stand_end_data_capture_thread(&o);
	assert_string_equal(o.log, "stop-raw-video deactivate-service cleanup-delay signal-deactivate set-stopping");
}

/* ------------------------------------------------------------------------ */
/* obs_output_signal_stop() */

static void signal_stop_steps(void **state)
{
	(void)state;
	struct stand_output o;

	/* no reconnect: the code kept for the stop signal, the capture ended */
	stand_init(&o, STREAM);
	stand_signal_stop(&o, CODE_DISCONNECTED);
	assert_string_equal(o.log, "log-frames stop-params create-end-thread signal-stop--2");
	assert_int_equal(o.stop_code, CODE_SUCCESS);

	/* no reconnect, a delay active and reconnecting: all three cleared
	 * before the end, so the delay ends whatever its restart refs */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.delay_capturing = true;
	o.reconnecting = true;
	o.delay_restart_refs = 1;
	stand_signal_stop(&o, CODE_ERROR);
	assert_string_equal(o.log, "log-frames stop-params create-end-thread signal-stop--4");
	assert_false(o.delay_active);
	assert_false(o.delay_capturing);
	assert_false(o.reconnecting);
	assert_int_equal(o.delay_restart_refs, 1);

	/* a reconnect: a delay takes a restart reference and keeps going, no
	 * stop signal */
	stand_init(&o, STREAM);
	o.can_reconnect = true;
	o.delay_active = true;
	stand_signal_stop(&o, CODE_DISCONNECTED);
	assert_string_equal(o.log, "set-stopping reconnect");
	assert_int_equal(o.delay_restart_refs, 1);
	assert_true(o.delay_active);
	assert_int_equal(o.stop_code, CODE_DISCONNECTED);

	/* a reconnect without a delay: the capture ends quietly */
	stand_init(&o, STREAM);
	o.can_reconnect = true;
	stand_signal_stop(&o, CODE_DISCONNECTED);
	assert_string_equal(o.log, "log-frames create-end-thread reconnect");
	assert_int_equal(o.stop_code, CODE_DISCONNECTED);

	/* a stream that failed to connect: the unstarted encoders shut down */
	stand_init(&o, STREAM);
	o.active = false;
	o.data_active = false;
	stand_signal_stop(&o, CODE_ERROR);
	assert_string_equal(o.log, "stop-params create-unstarted-thread signal-stop--4 set-stopping");

	/* invalid: nothing, the code kept */
	stand_init(&o, STREAM);
	o.valid = false;
	stand_signal_stop(&o, CODE_DISCONNECTED);
	assert_string_equal(o.log, "");
	assert_int_equal(o.stop_code, CODE_ERROR);
}

/* ------------------------------------------------------------------------ */
/* sequences */

static void stop_sequences(void **state)
{
	(void)state;
	struct stand_output o;

	/* a stream stops, its plugin ends the capture, the thread runs, and a
	 * second stop finds it inactive */
	stand_init(&o, STREAM);
	stand_stop(&o);
	stand_end_data_capture(&o);
	stand_run_thread(&o);
	assert_string_equal(o.log, "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2 log-frames "
				   "stop-params create-end-thread signal-stop--4 stop-video-0-interleave "
				   "stop-audio-0-interleave deactivate-service signal-deactivate set-stopping");
	clear_log(&o);
	stand_stop(&o);
	assert_string_equal(o.log, "");

	/* a second stop while the first is under way: nothing */
	stand_init(&o, RECORDING);
	stand_stop(&o);
	clear_log(&o);
	stand_stop(&o);
	assert_string_equal(o.log, "");
	/* but a forced one goes through */
	stand_force_stop(&o);
	assert_string_equal(o.log, "unpause reset-stopping info.stop-0 free-captions-0-0");

	/* an end while the last end-capture thread has not run: it is joined
	 * first */
	stand_init(&o, RECORDING);
	stand_end_data_capture(&o);
	o.data_active = true;
	clear_log(&o);
	stand_end_data_capture(&o);
	assert_string_equal(o.log, "log-frames join-end-thread stop-params create-end-thread signal-stop-0");

	/* a delayed stop, then the delay's end */
	stand_init(&o, STREAM);
	o.active_delay_ns = 1000000000ULL;
	o.delay_active = true;
	o.delay_capturing = true;
	stand_stop(&o);
	stand_end_data_capture(&o);
	stand_run_thread(&o);
	assert_string_equal(o.log, "delay-stop log-frames stop-params create-end-thread signal-stop--4 "
				   "stop-video-0-delay stop-audio-0-delay deactivate-service cleanup-delay "
				   "signal-deactivate set-stopping");
	assert_false(o.delay_active);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(flag_rules),
		cmocka_unit_test(delay_on),
		cmocka_unit_test(end_delay),
		cmocka_unit_test(thread_failed),
		cmocka_unit_test(shared_rules),
		cmocka_unit_test(stop_routes),
		cmocka_unit_test(stop_delayed),
		cmocka_unit_test(stop_reconnecting),
		cmocka_unit_test(force_stop),
		cmocka_unit_test(actual_stop_refusal),
		cmocka_unit_test(actual_stop_forced_delay),
		cmocka_unit_test(end_not_active),
		cmocka_unit_test(end_delay_route),
		cmocka_unit_test(end_full),
		cmocka_unit_test(end_thread_callbacks),
		cmocka_unit_test(end_thread_raw),
		cmocka_unit_test(signal_stop_steps),
		cmocka_unit_test(stop_sequences),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
