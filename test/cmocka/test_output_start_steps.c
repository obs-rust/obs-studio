#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-start.h"
#include "obs-output-raw-audio.h"
#include "obs-encoder-size.h"

/*
 * Characterization of the output start and begin-capture decisions of
 * libobs/obs-output.c, moved to libobs/obs-output-start.c before their Rust
 * port:
 * - obs_output_start() and obs_output_begin_data_capture_internal(): the
 *   service flag (obs_output_start_service), asked before the service gate
 *   and for the activation;
 * - obs_output_actual_start(): the delayed restart's reference
 *   (_drop_restart_ref);
 * - obs_output_initialize_encoders(): which encoders are initialized
 *   (_init_video, _init_audio);
 * - obs_output_begin_data_capture_internal(): the raw reset (_reset_raw)
 *   and the encoder pairing (_pair).
 * obs_output_start() asks obs_output_raw_audio_delay_on() whether the start
 * is delayed, and reset_raw_output() fills a raw output's audio from its
 * conversion with obs_encoder_size_audio_defaults(); both are pinned here
 * at those callers' values.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The bodies of obs_output_start(), obs_output_actual_start(),
 * obs_output_initialize_encoders(), begin_delayed_capture(),
 * reset_raw_output(), obs_output_begin_data_capture_internal() and
 * obs_output_begin_data_capture() are also copied below onto a stand-in
 * output (no thread, no lock, no service and no encoder: their results are
 * given, and every call, atomic store, signal and warning is recorded), so
 * every route over the atomics is pinned end to end: each of starting,
 * delay_active, active, reconnecting and delay_capturing set and clear, the
 * service gate when it cannot try and when initialize fails, a failed
 * info.start with and without restart refs, the encoder initialization while
 * active and delayed, the audio conversion with each field set alone, and
 * each signal that ends a begin.
 *
 * The functions are hidden in libobs, so this test compiles the C sources
 * into itself and calls only them, through their headers. The real-libobs
 * tests test_output_start_twice, test_output_failed_start_encoders,
 * test_output_delay, test_output_delay_restart, test_output_before_video,
 * test_raw_output_video_reset, test_output_interleave_restart and
 * test_rtmp_send_thread_leak cover the callers.
 *
 * Nothing here comes from outside OBS: the flags come from the output's
 * plugin, the delay and the conversion from the frontend.
 */

#define F_VIDEO 1u
#define F_AUDIO 2u
#define F_ENCODED 4u
#define F_SERVICE 8u

/* enum audio_format and enum speaker_layout values, as int */
#define FORMAT_UNKNOWN 0
#define FORMAT_16BIT 2
#define FORMAT_FLOAT 4
#define FORMAT_FLOAT_PLANAR 8
#define SPEAKERS_UNKNOWN 0
#define SPEAKERS_MONO 1
#define SPEAKERS_STEREO 2
#define SPEAKERS_5POINT1 6

static const long edge_long[] = {LONG_MIN, -2, -1, 0, 1, 2, 3, LONG_MAX};
#define N_EDGE_LONG (sizeof(edge_long) / sizeof(edge_long[0]))
static const uint32_t edge_u32[] = {0, 1, 2, 30, 0x7fffffffU, 0x80000000U, UINT32_MAX};
#define N_EDGE_U32 (sizeof(edge_u32) / sizeof(edge_u32[0]))

/* ------------------------------------------------------------------------ */
/* the constants and the flag rules */

static void constants(void **state)
{
	(void)state;
	/* OBS_OUTPUT_VIDEO, _AUDIO, _ENCODED, _SERVICE */
	assert_int_equal(OBS_OUTPUT_START_VIDEO, 1);
	assert_int_equal(OBS_OUTPUT_START_AUDIO, 2);
	assert_int_equal(OBS_OUTPUT_START_ENCODED, 4);
	assert_int_equal(OBS_OUTPUT_START_SERVICE, 8);
}

static void flag_rules(void **state)
{
	(void)state;
	/* every low 16-bit flag word, with and without high bits: each rule
	 * looks at its own bits only */
	static const uint32_t high[] = {0, 0x10000u, 0x80000000u, 0xffff0000u};
	for (size_t h = 0; h < sizeof(high) / sizeof(high[0]); h++) {
		for (uint32_t low = 0; low <= 0xffffu; low++) {
			const uint32_t f = high[h] | low;
			assert_int_equal(obs_output_start_service(f), (f & F_SERVICE) != 0);
			assert_int_equal(obs_output_start_init_video(f), (f & F_VIDEO) != 0);
			assert_int_equal(obs_output_start_init_audio(f), (f & F_AUDIO) != 0);
			assert_int_equal(obs_output_start_reset_raw(f), (f & F_ENCODED) == 0);
			assert_int_equal(obs_output_start_pair(f), (f & F_VIDEO) != 0 && (f & F_AUDIO) != 0);
		}
	}
	/* the common outputs: an encoded stream (AV, encoded, service), a
	 * recording (AV, encoded), a raw virtual camera (video only) */
	assert_true(obs_output_start_service(F_VIDEO | F_AUDIO | F_ENCODED | F_SERVICE));
	assert_false(obs_output_start_service(F_VIDEO | F_AUDIO | F_ENCODED));
	assert_false(obs_output_start_reset_raw(F_VIDEO | F_AUDIO | F_ENCODED));
	assert_true(obs_output_start_reset_raw(F_VIDEO));
	assert_true(obs_output_start_pair(F_VIDEO | F_AUDIO));
	assert_false(obs_output_start_pair(F_VIDEO));
	assert_false(obs_output_start_pair(F_AUDIO));
	assert_false(obs_output_start_pair(0));
}

static void drop_restart_ref(void **state)
{
	(void)state;
	assert_false(obs_output_start_drop_restart_ref(0));
	assert_true(obs_output_start_drop_restart_ref(1));
	assert_true(obs_output_start_drop_restart_ref(2));
	/* a negative count is dropped too: the test is "not 0", not "positive"
	 * (characterized, not endorsed; only the delay increments it, and only
	 * here is it decremented, after this test, so it never goes below 0) */
	assert_true(obs_output_start_drop_restart_ref(-1));
	for (size_t i = 0; i < N_EDGE_LONG; i++)
		assert_int_equal(obs_output_start_drop_restart_ref(edge_long[i]), edge_long[i] != 0);
}

static void delayed_start(void **state)
{
	(void)state;
	/* obs_output_start(): a delay that is not 0 starts through
	 * obs_output_delay_start() */
	assert_false(obs_output_raw_audio_delay_on(0));
	for (size_t i = 0; i < N_EDGE_U32; i++)
		assert_int_equal(obs_output_raw_audio_delay_on(edge_u32[i]), edge_u32[i] != 0);
}

/* reset_raw_output()'s fill, as the caller asks it */
static struct obs_encoder_audio_fill conversion_fill(int format, uint32_t rate, int speakers, int mix_format,
						     uint32_t mix_rate, int mix_speakers)
{
	const struct obs_encoder_audio_fill set = {.format = format, .samples_per_sec = rate, .speakers = speakers};
	const struct obs_encoder_audio_fill mix = {
		.format = mix_format,
		.samples_per_sec = mix_rate,
		.speakers = mix_speakers,
	};
	return obs_encoder_size_audio_defaults(&set, &mix);
}

static void audio_conversion_fields(void **state)
{
	(void)state;
	struct obs_encoder_audio_fill out;

	/* nothing set: the mix's */
	out = conversion_fill(FORMAT_UNKNOWN, 0, SPEAKERS_UNKNOWN, FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO);
	assert_int_equal(out.format, FORMAT_FLOAT_PLANAR);
	assert_int_equal(out.samples_per_sec, 48000);
	assert_int_equal(out.speakers, SPEAKERS_STEREO);

	/* each field set alone replaces only the mix's */
	out = conversion_fill(FORMAT_UNKNOWN, 44100, SPEAKERS_UNKNOWN, FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO);
	assert_int_equal(out.format, FORMAT_FLOAT_PLANAR);
	assert_int_equal(out.samples_per_sec, 44100);
	assert_int_equal(out.speakers, SPEAKERS_STEREO);
	out = conversion_fill(FORMAT_16BIT, 0, SPEAKERS_UNKNOWN, FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO);
	assert_int_equal(out.format, FORMAT_16BIT);
	assert_int_equal(out.samples_per_sec, 48000);
	assert_int_equal(out.speakers, SPEAKERS_STEREO);
	out = conversion_fill(FORMAT_UNKNOWN, 0, SPEAKERS_MONO, FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO);
	assert_int_equal(out.format, FORMAT_FLOAT_PLANAR);
	assert_int_equal(out.samples_per_sec, 48000);
	assert_int_equal(out.speakers, SPEAKERS_MONO);

	/* all set */
	out = conversion_fill(FORMAT_16BIT, 44100, SPEAKERS_5POINT1, FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO);
	assert_int_equal(out.format, FORMAT_16BIT);
	assert_int_equal(out.samples_per_sec, 44100);
	assert_int_equal(out.speakers, SPEAKERS_5POINT1);

	/* a mix of 0 and unknown fields stays so where nothing is set
	 * (characterized, not endorsed; a mix always has all three) */
	out = conversion_fill(FORMAT_UNKNOWN, 0, SPEAKERS_UNKNOWN, FORMAT_UNKNOWN, 0, SPEAKERS_UNKNOWN);
	assert_int_equal(out.format, FORMAT_UNKNOWN);
	assert_int_equal(out.samples_per_sec, 0);
	assert_int_equal(out.speakers, SPEAKERS_UNKNOWN);

	/* the edges of the rate: only 0 keeps the mix's */
	for (size_t i = 0; i < N_EDGE_U32; i++) {
		out = conversion_fill(FORMAT_UNKNOWN, edge_u32[i], SPEAKERS_UNKNOWN, FORMAT_FLOAT, 48000,
				      SPEAKERS_STEREO);
		assert_int_equal(out.samples_per_sec, edge_u32[i] ? edge_u32[i] : 48000);
	}
	/* any format or speakers value that is not unknown is taken as it is,
	 * even one past the enum (characterized, not endorsed; the frontend
	 * only sets known values) */
	out = conversion_fill(-1, 0, 99, FORMAT_FLOAT, 48000, SPEAKERS_STEREO);
	assert_int_equal(out.format, -1);
	assert_int_equal(out.speakers, 99);
}

/* ------------------------------------------------------------------------ */
/* the callers, copied onto a stand-in output */

struct stand_track {
	bool present;
	int locks;
	double caption_timestamp;
	size_t caption_data_size;
};

struct stand_output {
	/* the output */
	bool has_data;
	uint32_t flags;
	uint32_t delay_sec;
	bool has_service;
	bool has_audio;
	bool audio_conversion_set;
	int conv_format;
	uint32_t conv_rate;
	int conv_speakers;
	bool has_last_error;
	int stop_code;
	struct stand_track tracks[3];
	/* the atomics */
	bool starting, active, reconnecting, delay_active, delay_capturing, data_active;
	long delay_restart_refs;
	/* what the callers wrote */
	uint32_t starting_drawn_count, starting_lagged_count;
	int total_frames;
	uint32_t sample_rate;
	int audio_format;
	int audio_speakers;
	uint64_t total_audio_frames;
	uint64_t audio_start_ts, video_start_ts;
	/* the results of the calls the C keeps */
	bool valid;
	bool can_try, service_init_ok;
	bool start_ok;
	bool delay_start_ok;
	bool init_video_ok, init_audio_ok;
	bool can_begin;
	/* the mix */
	int mix_format;
	uint32_t mix_rate;
	int mix_speakers;
	/* the record */
	char log[1024];
};

static void rec(struct stand_output *o, const char *what)
{
	const size_t n = strlen(o->log);
	snprintf(o->log + n, sizeof(o->log) - n, "%s%s", n ? " " : "", what);
}

static void stand_init(struct stand_output *o, uint32_t flags)
{
	memset(o, 0, sizeof(*o));
	o->has_data = true;
	o->flags = flags;
	o->has_service = true;
	o->has_audio = true;
	o->valid = true;
	o->can_try = true;
	o->service_init_ok = true;
	o->start_ok = true;
	o->delay_start_ok = true;
	o->init_video_ok = true;
	o->init_audio_ok = true;
	o->can_begin = true;
	o->mix_format = FORMAT_FLOAT_PLANAR;
	o->mix_rate = 48000;
	o->mix_speakers = SPEAKERS_STEREO;
	o->total_audio_frames = 77;
	o->audio_start_ts = 5;
	o->video_start_ts = 6;
	o->total_frames = 9;
}

/* obs_output_actual_start() */
static bool stand_actual_start(struct stand_output *o)
{
	bool success = false;

	rec(o, "wait-stopping");
	o->stop_code = 0;
	if (o->has_last_error) {
		rec(o, "free-error");
		o->has_last_error = false;
	}

	o->starting = true;
	if (o->has_data) {
		rec(o, "info.start");
		success = o->start_ok;
	}

	if (success) {
		rec(o, "frame-counts");
		o->starting_drawn_count = 100;
		o->starting_lagged_count = 3;
	}

	if (!success)
		o->starting = false;

	if (obs_output_start_drop_restart_ref(o->delay_restart_refs)) {
		rec(o, "drop-restart-ref");
		o->delay_restart_refs--;
	}

	for (size_t i = 0; i < 3; i++) {
		struct stand_track *t = &o->tracks[i];
		if (!t->present)
			continue;
		t->locks++;
		t->caption_timestamp = 0;
		t->caption_data_size = 0;
	}

	return success;
}

/* obs_output_start() */
static bool stand_start(struct stand_output *o)
{
	if (!o->valid)
		return false;
	if (!o->has_data)
		return false;

	if (o->starting && !o->delay_active) {
		rec(o, "warn-already-starting");
		return false;
	}

	if (obs_output_start_service(o->flags)) {
		rec(o, "can-try");
		if (!o->can_try)
			return false;
		rec(o, "service-init");
		if (!o->service_init_ok)
			return false;
	}

	if (obs_output_raw_audio_delay_on(o->delay_sec)) {
		rec(o, "delay-start");
		return o->delay_start_ok;
	} else {
		if (stand_actual_start(o)) {
			rec(o, "signal-starting");
			return true;
		}

		return false;
	}
}

/* obs_output_initialize_encoders() */
static bool stand_initialize_encoders(struct stand_output *o)
{
	if (!o->valid)
		return false;
	if ((o->flags & F_ENCODED) == 0) {
		rec(o, "warn-raw");
		return false;
	}
	if (o->active)
		return o->delay_active;

	if (obs_output_start_init_video(o->flags)) {
		rec(o, "init-video");
		if (!o->init_video_ok)
			return false;
	}
	if (obs_output_start_init_audio(o->flags)) {
		rec(o, "init-audio");
		if (!o->init_audio_ok)
			return false;
	}

	return true;
}

/* begin_delayed_capture() */
static bool stand_begin_delayed_capture(struct stand_output *o)
{
	if (o->delay_capturing)
		return false;

	rec(o, "reset-packets");
	o->delay_capturing = true;

	if (o->reconnecting) {
		rec(o, "signal-reconnect-success");
		o->reconnecting = false;
	} else {
		rec(o, "signal-start");
	}

	return true;
}

/* reset_raw_output() */
static void stand_reset_raw_output(struct stand_output *o)
{
	rec(o, "clear-raw-audio");

	if (o->has_audio) {
		struct obs_encoder_audio_fill info = {
			.format = o->mix_format,
			.samples_per_sec = o->mix_rate,
			.speakers = o->mix_speakers,
		};

		if (o->audio_conversion_set) {
			const struct obs_encoder_audio_fill set = {
				.format = o->conv_format,
				.samples_per_sec = o->conv_rate,
				.speakers = o->conv_speakers,
			};
			info = obs_encoder_size_audio_defaults(&set, &info);
		}

		o->sample_rate = info.samples_per_sec;
		o->audio_format = info.format;
		o->audio_speakers = info.speakers;
		o->total_audio_frames = 0;
	}

	o->audio_start_ts = 0;
	o->video_start_ts = 0;

	rec(o, "pause-reset");
}

/* obs_output_begin_data_capture_internal() */
static bool stand_begin_internal(struct stand_output *o)
{
	if (o->delay_active)
		return stand_begin_delayed_capture(o);
	if (o->active)
		return false;

	o->total_frames = 0;

	if (obs_output_start_reset_raw(o->flags))
		stand_reset_raw_output(o);

	rec(o, "can-begin");
	if (!o->can_begin)
		return false;

	if (obs_output_start_pair(o->flags))
		rec(o, "pair");

	o->data_active = true;
	rec(o, "hook");
	/* hook_data_capture(): an encoded output with a delay turns it on */
	if ((o->flags & F_ENCODED) != 0 && o->delay_sec)
		o->delay_active = true;

	rec(o, "batch-size");

	if (obs_output_start_service(o->flags))
		rec(o, "service-activate");

	rec(o, "signal-activate");
	o->active = true;

	if (o->reconnecting) {
		rec(o, "signal-reconnect-success");
		o->reconnecting = false;

	} else if (o->delay_active) {
		rec(o, "signal-starting");

	} else {
		rec(o, "signal-start");
	}

	return true;
}

/* obs_output_begin_data_capture() */
static bool stand_begin(struct stand_output *o)
{
	if (!o->valid)
		return false;

	bool success = stand_begin_internal(o);

	o->starting = false;
	return success;
}

#define STREAM (F_VIDEO | F_AUDIO | F_ENCODED | F_SERVICE)
#define RECORDING (F_VIDEO | F_AUDIO | F_ENCODED)
#define RAW_VIDEO (F_VIDEO)

/* ------------------------------------------------------------------------ */
/* obs_output_start() */

static void start_routes(void **state)
{
	(void)state;
	struct stand_output o;

	/* a stream: the service gate, then the actual start and "starting" */
	stand_init(&o, STREAM);
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "can-try service-init wait-stopping info.start frame-counts signal-starting");
	assert_true(o.starting);
	assert_int_equal(o.starting_drawn_count, 100);

	/* a recording: no service */
	stand_init(&o, RECORDING);
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "wait-stopping info.start frame-counts signal-starting");

	/* invalid, and no data: refused before anything */
	stand_init(&o, STREAM);
	o.valid = false;
	assert_false(stand_start(&o));
	assert_string_equal(o.log, "");
	stand_init(&o, STREAM);
	o.has_data = false;
	assert_false(stand_start(&o));
	assert_string_equal(o.log, "");
}

static void start_already_starting(void **state)
{
	(void)state;
	struct stand_output o;

	/* starting and no delay: refused with a warning, start() not run */
	stand_init(&o, STREAM);
	o.starting = true;
	assert_false(stand_start(&o));
	assert_string_equal(o.log, "warn-already-starting");
	assert_true(o.starting);

	/* starting with a delay active: a delayed output handles its own
	 * restarts, so the start goes on */
	stand_init(&o, STREAM);
	o.starting = true;
	o.delay_active = true;
	o.delay_sec = 10;
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "can-try service-init delay-start");

	/* a delay active but not starting: goes on */
	stand_init(&o, RECORDING);
	o.delay_active = true;
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "wait-stopping info.start frame-counts signal-starting");

	/* active, reconnecting and delay_capturing do not refuse the start:
	 * only starting does here (characterized, not endorsed; the plugin's
	 * start() decides) */
	stand_init(&o, RECORDING);
	o.active = true;
	o.reconnecting = true;
	o.delay_capturing = true;
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "wait-stopping info.start frame-counts signal-starting");
}

static void start_service_gate(void **state)
{
	(void)state;
	struct stand_output o;

	/* cannot try: refused, initialize not called */
	stand_init(&o, STREAM);
	o.can_try = false;
	assert_false(stand_start(&o));
	assert_string_equal(o.log, "can-try");
	assert_false(o.starting);

	/* initialize fails: refused */
	stand_init(&o, STREAM);
	o.service_init_ok = false;
	assert_false(stand_start(&o));
	assert_string_equal(o.log, "can-try service-init");

	/* no service flag: neither is asked, whatever they would say */
	stand_init(&o, RECORDING);
	o.can_try = false;
	o.service_init_ok = false;
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "wait-stopping info.start frame-counts signal-starting");

	/* the service flag on a raw output asks the service too */
	stand_init(&o, F_VIDEO | F_SERVICE);
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "can-try service-init wait-stopping info.start frame-counts signal-starting");
}

static void start_delayed(void **state)
{
	(void)state;
	struct stand_output o;

	/* a delay: obs_output_delay_start()'s result, no "starting" here */
	stand_init(&o, STREAM);
	o.delay_sec = 20;
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "can-try service-init delay-start");
	assert_false(o.starting);
	stand_init(&o, STREAM);
	o.delay_sec = UINT32_MAX;
	o.delay_start_ok = false;
	assert_false(stand_start(&o));
	assert_string_equal(o.log, "can-try service-init delay-start");

	/* the service gate comes first */
	stand_init(&o, STREAM);
	o.delay_sec = 1;
	o.can_try = false;
	assert_false(stand_start(&o));
	assert_string_equal(o.log, "can-try");

	/* a raw output with a delay also goes through the delayed start
	 * (characterized, not endorsed: the delay applies to encoded packets) */
	stand_init(&o, RAW_VIDEO);
	o.delay_sec = 5;
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "delay-start");
}

/* ------------------------------------------------------------------------ */
/* obs_output_actual_start() */

static void actual_start_steps(void **state)
{
	(void)state;
	struct stand_output o;

	/* success: the frame counts, starting kept, no restart refs */
	stand_init(&o, RECORDING);
	o.stop_code = 3;
	o.has_last_error = true;
	o.tracks[0].present = true;
	o.tracks[0].caption_timestamp = 12.5;
	o.tracks[0].caption_data_size = 40;
	o.tracks[2].present = true;
	o.tracks[2].caption_data_size = 8;
	assert_true(stand_actual_start(&o));
	assert_string_equal(o.log, "wait-stopping free-error info.start frame-counts");
	assert_int_equal(o.stop_code, 0);
	assert_false(o.has_last_error);
	assert_true(o.starting);
	assert_int_equal(o.delay_restart_refs, 0);
	/* each caption track is reset under its mutex, whatever the result */
	assert_int_equal(o.tracks[0].locks, 1);
	assert_true(o.tracks[0].caption_timestamp == 0);
	assert_int_equal(o.tracks[0].caption_data_size, 0);
	assert_int_equal(o.tracks[1].locks, 0);
	assert_int_equal(o.tracks[2].locks, 1);
	assert_int_equal(o.tracks[2].caption_data_size, 0);

	/* failure: starting cleared, no frame counts */
	stand_init(&o, RECORDING);
	o.start_ok = false;
	o.tracks[1].present = true;
	o.tracks[1].caption_data_size = 4;
	assert_false(stand_actual_start(&o));
	assert_string_equal(o.log, "wait-stopping info.start");
	assert_false(o.starting);
	assert_int_equal(o.starting_drawn_count, 0);
	assert_int_equal(o.tracks[1].caption_data_size, 0);

	/* no data: start() not run, a failure */
	stand_init(&o, RECORDING);
	o.has_data = false;
	assert_false(stand_actual_start(&o));
	assert_string_equal(o.log, "wait-stopping");
	assert_false(o.starting);
}

static void actual_start_restart_refs(void **state)
{
	(void)state;
	struct stand_output o;

	/* a delayed restart's reference is dropped after start(), on success
	 * and on failure */
	stand_init(&o, STREAM);
	o.delay_restart_refs = 2;
	assert_true(stand_actual_start(&o));
	assert_string_equal(o.log, "wait-stopping info.start frame-counts drop-restart-ref");
	assert_int_equal(o.delay_restart_refs, 1);

	stand_init(&o, STREAM);
	o.delay_restart_refs = 1;
	o.start_ok = false;
	assert_false(stand_actual_start(&o));
	assert_string_equal(o.log, "wait-stopping info.start drop-restart-ref");
	assert_int_equal(o.delay_restart_refs, 0);
	assert_false(o.starting);

	/* only one per start */
	stand_init(&o, STREAM);
	o.delay_restart_refs = 3;
	assert_true(stand_actual_start(&o));
	assert_true(stand_actual_start(&o));
	assert_int_equal(o.delay_restart_refs, 1);

	/* none: kept at 0 */
	stand_init(&o, STREAM);
	o.start_ok = false;
	assert_false(stand_actual_start(&o));
	assert_int_equal(o.delay_restart_refs, 0);

	/* a negative count is decremented too (characterized, not endorsed;
	 * never reached) */
	stand_init(&o, STREAM);
	o.delay_restart_refs = -1;
	assert_true(stand_actual_start(&o));
	assert_int_equal(o.delay_restart_refs, -2);
}

/* ------------------------------------------------------------------------ */
/* obs_output_initialize_encoders() */

static void initialize_encoders(void **state)
{
	(void)state;
	struct stand_output o;

	/* a raw output: warned and refused */
	stand_init(&o, RAW_VIDEO);
	assert_false(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "warn-raw");

	/* video, then audio */
	stand_init(&o, RECORDING);
	assert_true(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "init-video init-audio");
	stand_init(&o, F_VIDEO | F_ENCODED);
	assert_true(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "init-video");
	stand_init(&o, F_AUDIO | F_ENCODED);
	assert_true(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "init-audio");

	/* an encoded output with neither flag initializes nothing and succeeds
	 * (characterized, not endorsed: no such output exists) */
	stand_init(&o, F_ENCODED);
	assert_true(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "");

	/* a video failure stops before the audio */
	stand_init(&o, RECORDING);
	o.init_video_ok = false;
	assert_false(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "init-video");
	stand_init(&o, RECORDING);
	o.init_audio_ok = false;
	assert_false(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "init-video init-audio");
}

static void initialize_encoders_active(void **state)
{
	(void)state;
	struct stand_output o;

	/* active: nothing initialized; true only for a delayed output (its
	 * restart), false otherwise */
	stand_init(&o, STREAM);
	o.active = true;
	assert_false(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "");
	stand_init(&o, STREAM);
	o.active = true;
	o.delay_active = true;
	assert_true(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "");

	/* delay_active without active: initialized as usual */
	stand_init(&o, STREAM);
	o.delay_active = true;
	assert_true(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "init-video init-audio");

	/* the raw refusal comes before the active test */
	stand_init(&o, RAW_VIDEO);
	o.active = true;
	o.delay_active = true;
	assert_false(stand_initialize_encoders(&o));
	assert_string_equal(o.log, "warn-raw");
}

/* ------------------------------------------------------------------------ */
/* reset_raw_output() */

static void reset_raw_audio(void **state)
{
	(void)state;
	struct stand_output o;

	/* no conversion: the mix's */
	stand_init(&o, RAW_VIDEO | F_AUDIO);
	stand_reset_raw_output(&o);
	assert_string_equal(o.log, "clear-raw-audio pause-reset");
	assert_int_equal(o.sample_rate, 48000);
	assert_int_equal(o.audio_format, FORMAT_FLOAT_PLANAR);
	assert_int_equal(o.audio_speakers, SPEAKERS_STEREO);
	assert_int_equal(o.total_audio_frames, 0);
	assert_int_equal(o.audio_start_ts, 0);
	assert_int_equal(o.video_start_ts, 0);

	/* a conversion that is not set is ignored, whatever it holds */
	stand_init(&o, RAW_VIDEO | F_AUDIO);
	o.conv_format = FORMAT_16BIT;
	o.conv_rate = 44100;
	o.conv_speakers = SPEAKERS_MONO;
	stand_reset_raw_output(&o);
	assert_int_equal(o.sample_rate, 48000);
	assert_int_equal(o.audio_format, FORMAT_FLOAT_PLANAR);
	assert_int_equal(o.audio_speakers, SPEAKERS_STEREO);

	/* set, each field alone */
	stand_init(&o, RAW_VIDEO | F_AUDIO);
	o.audio_conversion_set = true;
	o.conv_rate = 44100;
	stand_reset_raw_output(&o);
	assert_int_equal(o.sample_rate, 44100);
	assert_int_equal(o.audio_format, FORMAT_FLOAT_PLANAR);
	assert_int_equal(o.audio_speakers, SPEAKERS_STEREO);
	stand_init(&o, RAW_VIDEO | F_AUDIO);
	o.audio_conversion_set = true;
	o.conv_format = FORMAT_16BIT;
	stand_reset_raw_output(&o);
	assert_int_equal(o.sample_rate, 48000);
	assert_int_equal(o.audio_format, FORMAT_16BIT);
	assert_int_equal(o.audio_speakers, SPEAKERS_STEREO);
	stand_init(&o, RAW_VIDEO | F_AUDIO);
	o.audio_conversion_set = true;
	o.conv_speakers = SPEAKERS_MONO;
	stand_reset_raw_output(&o);
	assert_int_equal(o.sample_rate, 48000);
	assert_int_equal(o.audio_format, FORMAT_FLOAT_PLANAR);
	assert_int_equal(o.audio_speakers, SPEAKERS_MONO);

	/* set with 0 and unknown: the mix's */
	stand_init(&o, RAW_VIDEO | F_AUDIO);
	o.audio_conversion_set = true;
	stand_reset_raw_output(&o);
	assert_int_equal(o.sample_rate, 48000);
	assert_int_equal(o.audio_format, FORMAT_FLOAT_PLANAR);
	assert_int_equal(o.audio_speakers, SPEAKERS_STEREO);

	/* no audio: only the timestamps and the pause (sample_rate and the
	 * frame count are kept) */
	stand_init(&o, RAW_VIDEO);
	o.has_audio = false;
	o.sample_rate = 32000;
	stand_reset_raw_output(&o);
	assert_string_equal(o.log, "clear-raw-audio pause-reset");
	assert_int_equal(o.sample_rate, 32000);
	assert_int_equal(o.total_audio_frames, 77);
	assert_int_equal(o.audio_start_ts, 0);
	assert_int_equal(o.video_start_ts, 0);
}

/* ------------------------------------------------------------------------ */
/* obs_output_begin_data_capture() and _internal() */

static void begin_routes(void **state)
{
	(void)state;
	struct stand_output o;

	/* a stream: pair, hook, activate the service, "start" */
	stand_init(&o, STREAM);
	o.starting = true;
	assert_true(stand_begin(&o));
	assert_string_equal(o.log, "can-begin pair hook batch-size service-activate signal-activate signal-start");
	assert_true(o.active);
	assert_true(o.data_active);
	assert_false(o.starting);
	assert_int_equal(o.total_frames, 0);

	/* a recording: no service */
	stand_init(&o, RECORDING);
	assert_true(stand_begin(&o));
	assert_string_equal(o.log, "can-begin pair hook batch-size signal-activate signal-start");

	/* a raw video output: the raw reset first, no pairing */
	stand_init(&o, RAW_VIDEO);
	assert_true(stand_begin(&o));
	assert_string_equal(o.log,
			    "clear-raw-audio pause-reset can-begin hook batch-size signal-activate signal-start");

	/* a raw AV output is paired too (characterized, not endorsed:
	 * pair_encoders() finds no encoders on a raw output) */
	stand_init(&o, F_VIDEO | F_AUDIO);
	assert_true(stand_begin(&o));
	assert_string_equal(o.log,
			    "clear-raw-audio pause-reset can-begin pair hook batch-size signal-activate signal-start");

	/* an encoded audio-only output: no pairing */
	stand_init(&o, F_AUDIO | F_ENCODED);
	assert_true(stand_begin(&o));
	assert_string_equal(o.log, "can-begin hook batch-size signal-activate signal-start");
}

static void begin_refusals(void **state)
{
	(void)state;
	struct stand_output o;

	/* active: refused before anything, starting still cleared */
	stand_init(&o, STREAM);
	o.active = true;
	o.starting = true;
	o.total_frames = 9;
	assert_false(stand_begin(&o));
	assert_string_equal(o.log, "");
	assert_false(o.starting);
	assert_int_equal(o.total_frames, 9);

	/* the capture check fails: total_frames already reset (and a raw
	 * output's audio), data_active not set, starting cleared */
	stand_init(&o, RAW_VIDEO);
	o.can_begin = false;
	o.starting = true;
	assert_false(stand_begin(&o));
	assert_string_equal(o.log, "clear-raw-audio pause-reset can-begin");
	assert_int_equal(o.total_frames, 0);
	assert_false(o.data_active);
	assert_false(o.active);
	assert_false(o.starting);

	/* invalid: starting not touched */
	stand_init(&o, STREAM);
	o.valid = false;
	o.starting = true;
	assert_false(stand_begin(&o));
	assert_true(o.starting);
}

static void begin_signals(void **state)
{
	(void)state;
	struct stand_output o;

	/* reconnecting: "reconnect_success", the flag cleared */
	stand_init(&o, STREAM);
	o.reconnecting = true;
	assert_true(stand_begin(&o));
	assert_string_equal(o.log,
			    "can-begin pair hook batch-size service-activate signal-activate signal-reconnect-success");
	assert_false(o.reconnecting);

	/* a delay turned on by the hookup: "starting" (the delay signals
	 * "start" when its first packet goes out) */
	stand_init(&o, STREAM);
	o.delay_sec = 30;
	assert_true(stand_begin(&o));
	assert_string_equal(o.log, "can-begin pair hook batch-size service-activate signal-activate signal-starting");
	assert_true(o.delay_active);

	/* reconnecting wins over the delay */
	stand_init(&o, STREAM);
	o.delay_sec = 30;
	o.reconnecting = true;
	assert_true(stand_begin(&o));
	assert_string_equal(o.log,
			    "can-begin pair hook batch-size service-activate signal-activate signal-reconnect-success");

	/* a raw output with a delay: no delay is turned on, "start" */
	stand_init(&o, RAW_VIDEO);
	o.delay_sec = 30;
	assert_true(stand_begin(&o));
	assert_false(o.delay_active);
	assert_string_equal(o.log,
			    "clear-raw-audio pause-reset can-begin hook batch-size signal-activate signal-start");
}

static void begin_delayed(void **state)
{
	(void)state;
	struct stand_output o;

	/* delay_active (a delayed restart): the delayed capture, nothing else */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.active = true;
	o.starting = true;
	assert_true(stand_begin(&o));
	assert_string_equal(o.log, "reset-packets signal-start");
	assert_true(o.delay_capturing);
	assert_false(o.starting);
	assert_int_equal(o.total_frames, 9);

	/* reconnecting: "reconnect_success", the flag cleared */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.reconnecting = true;
	assert_true(stand_begin(&o));
	assert_string_equal(o.log, "reset-packets signal-reconnect-success");
	assert_false(o.reconnecting);

	/* already capturing: refused, starting cleared */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.delay_capturing = true;
	o.starting = true;
	assert_false(stand_begin(&o));
	assert_string_equal(o.log, "");
	assert_false(o.starting);

	/* delay_active comes before active and the capture check */
	stand_init(&o, STREAM);
	o.delay_active = true;
	o.can_begin = false;
	assert_true(stand_begin(&o));
	assert_string_equal(o.log, "reset-packets signal-start");
}

static void start_then_begin(void **state)
{
	(void)state;
	struct stand_output o;

	/* a stream's start, then the plugin's begin: "starting" then "start",
	 * starting set in between and cleared after active */
	stand_init(&o, STREAM);
	assert_true(stand_start(&o));
	assert_true(o.starting);
	assert_true(stand_begin(&o));
	assert_false(o.starting);
	assert_true(o.active);
	assert_string_equal(o.log, "can-try service-init wait-stopping info.start frame-counts signal-starting "
				   "can-begin pair hook batch-size service-activate signal-activate signal-start");

	/* a second start while the first is still starting is refused */
	stand_init(&o, STREAM);
	assert_true(stand_start(&o));
	assert_false(stand_start(&o));
	assert_string_equal(
		o.log,
		"can-try service-init wait-stopping info.start frame-counts signal-starting warn-already-starting");

	/* after a failed start, a start goes again */
	stand_init(&o, STREAM);
	o.start_ok = false;
	assert_false(stand_start(&o));
	o.start_ok = true;
	assert_true(stand_start(&o));
	assert_string_equal(o.log, "can-try service-init wait-stopping info.start "
				   "can-try service-init wait-stopping info.start frame-counts signal-starting");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(flag_rules),
		cmocka_unit_test(drop_restart_ref),
		cmocka_unit_test(delayed_start),
		cmocka_unit_test(audio_conversion_fields),
		cmocka_unit_test(start_routes),
		cmocka_unit_test(start_already_starting),
		cmocka_unit_test(start_service_gate),
		cmocka_unit_test(start_delayed),
		cmocka_unit_test(actual_start_steps),
		cmocka_unit_test(actual_start_restart_refs),
		cmocka_unit_test(initialize_encoders),
		cmocka_unit_test(initialize_encoders_active),
		cmocka_unit_test(reset_raw_audio),
		cmocka_unit_test(begin_routes),
		cmocka_unit_test(begin_refusals),
		cmocka_unit_test(begin_signals),
		cmocka_unit_test(begin_delayed),
		cmocka_unit_test(start_then_begin),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
