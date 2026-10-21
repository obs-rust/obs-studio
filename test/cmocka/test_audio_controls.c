#include <math.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/platform.h>

/* Characterization of libobs/obs-audio-controls.c.h before its port (#1488).
 * The expected values are what that file does today. Cases marked
 * "characterized, not endorsed" pin odd behavior of that C.
 *
 * Not called, because the C crashes or does not return:
 * - obs_volmeter_get_nr_channels(NULL)
 * - obs_volmeter_set_peak_meter_type(NULL, ...)
 * - obs_fader_db_to_def(NULL)
 *
 * The unaligned-plane path (peak forced to 1, a printf, that plane skipped)
 * is not reached through obs_source_output_audio. The copy the source keeps
 * is a bmalloc buffer, which is aligned.
 */

#define SOURCE_ID "audio_controls_char_input"
#define RATE 48000
#define TONE_FRAMES 8

static bool obs_up;
static int null_fader_logs;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	char line[256];

	(void)level;
	(void)param;
	vsnprintf(line, sizeof(line), format, args);
	if (strstr(line, "Null 'fader'"))
		null_fader_logs++;
}

static void assert_db(float got, float expect)
{
	if (isnan(expect)) {
		assert_true(isnan(got));
		return;
	}
	if (isinf(expect)) {
		assert_true(isinf(got) && signbit(got) == signbit(expect));
		return;
	}
	assert_float_equal(got, expect, 0.0001);
}

static const char *source_name(void *type_data)
{
	(void)type_data;
	return "audio controls";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return (void *)1;
}

static void source_destroy(void *data)
{
	(void)data;
}

static void register_source(void)
{
	struct obs_source_info info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
	};

	obs_register_source(&info);
}

static int source_setup(void **state)
{
	struct obs_audio_info audio = {
		.samples_per_sec = RATE,
		.speakers = SPEAKERS_STEREO,
	};

	(void)state;
	base_set_log_handler(log_handler, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		print_error("obs_startup failed\n");
		return -1;
	}
	register_source();
	if (!obs_reset_audio(&audio)) {
		/* cmocka skips the teardown of a failed setup: shut down here, or
		 * every later case fails in obs_startup too. */
		print_error("obs_reset_audio failed\n");
		obs_shutdown();
		obs_up = false;
		return -1;
	}
	return 0;
}

static void no_task(void *param)
{
	(void)param;
}

static int source_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
		obs_shutdown();
		obs_up = false;
	}
	base_set_log_handler(NULL, NULL);
	return 0;
}

static obs_source_t *make_source(void)
{
	obs_source_t *source = obs_source_create(SOURCE_ID, "tone", NULL, NULL);

	assert_non_null(source);
	return source;
}

static void null_pointers_test(void **state)
{
	(void)state;

	assert_false(obs_fader_set_db(NULL, -3.0f));
	assert_db(obs_fader_get_db(NULL), 0.0f);
	assert_false(obs_fader_set_deflection(NULL, 0.5f));
	assert_db(obs_fader_get_deflection(NULL), 0.0f);
	assert_false(obs_fader_set_mul(NULL, 0.5f));
	assert_db(obs_fader_get_mul(NULL), 0.0f);
	assert_false(obs_fader_attach_source(NULL, NULL));
	obs_fader_detach_source(NULL);
	obs_fader_destroy(NULL);

	assert_false(obs_volmeter_attach_source(NULL, NULL));
	obs_volmeter_detach_source(NULL);
	obs_volmeter_destroy(NULL);

	assert_db(obs_mul_to_db(0.0f), -INFINITY);
	assert_db(obs_mul_to_db(1.0f), 0.0f);
	assert_db(obs_db_to_mul(0.0f), 1.0f);
	assert_db(obs_db_to_mul(-INFINITY), 0.0f);
}

static void fresh_fader_is_unity_test(void **state)
{
	enum obs_fader_type types[3] = {OBS_FADER_CUBIC, OBS_FADER_IEC, OBS_FADER_LOG};
	int i;

	(void)state;

	for (i = 0; i < 3; i++) {
		long before = bnum_allocs();
		obs_fader_t *fader = obs_fader_create(types[i]);

		assert_non_null(fader);
		assert_int_equal(bnum_allocs(), before + 1);
		assert_db(obs_fader_get_db(fader), 0.0f);
		assert_db(obs_fader_get_deflection(fader), 1.0f);
		assert_db(obs_fader_get_mul(fader), 1.0f);
		assert_db(obs_fader_db_to_def(fader)(0.0f), 1.0f);

		obs_fader_destroy(fader);
		assert_int_equal(bnum_allocs(), before);
	}
}

/* characterized, not endorsed: an unknown type is freed and returns null.
 * A volmeter stores any type and never reads it. */
static void bad_type_test(void **state)
{
	long before = bnum_allocs();
	obs_fader_t *fader = obs_fader_create((enum obs_fader_type)99);
	obs_volmeter_t *meter = obs_volmeter_create((enum obs_fader_type)99);

	(void)state;
	assert_null(fader);
	assert_non_null(meter);
	assert_int_equal(bnum_allocs(), before + 1);

	obs_volmeter_destroy(meter);
	assert_int_equal(bnum_allocs(), before);
}

static void cubic_curve_test(void **state)
{
	obs_fader_t *fader = obs_fader_create(OBS_FADER_CUBIC);

	(void)state;
	assert_true(obs_fader_set_deflection(fader, 1.0f));
	assert_db(obs_fader_get_db(fader), 0.0f);

	assert_true(obs_fader_set_deflection(fader, 0.0f));
	assert_db(obs_fader_get_db(fader), -INFINITY);
	assert_db(obs_fader_get_mul(fader), 0.0f);

	assert_true(obs_fader_set_deflection(fader, 0.5f));
	assert_db(obs_fader_get_db(fader), -18.0618f);
	assert_db(obs_fader_get_deflection(fader), 0.5f);

	/* Above 1 the curve is positive dB and the fader clamps to 0. */
	assert_false(obs_fader_set_deflection(fader, 1.5f));
	assert_db(obs_fader_get_db(fader), 0.0f);
	assert_db(obs_fader_get_deflection(fader), 1.0f);

	/* min_db is -infinity, so a finite value below 0 is stored as given. */
	assert_true(obs_fader_set_db(fader, -1000.0f));
	assert_db(obs_fader_get_db(fader), -1000.0f);

	/* characterized, not endorsed: NaN is stored. Deflection of NaN is 0. */
	assert_true(obs_fader_set_db(fader, NAN));
	assert_db(obs_fader_get_db(fader), NAN);
	assert_db(obs_fader_get_deflection(fader), 0.0f);

	assert_true(obs_fader_set_mul(fader, 0.5f));
	assert_db(obs_fader_get_db(fader), obs_mul_to_db(0.5f));
	assert_db(obs_fader_get_mul(fader), 0.5f);

	assert_true(obs_fader_set_mul(fader, 0.0f));
	assert_db(obs_fader_get_db(fader), -INFINITY);
	assert_db(obs_fader_get_mul(fader), 0.0f);

	/* Above 0 dB the cubic fader clamps and reports it. */
	assert_false(obs_fader_set_mul(fader, 2.0f));
	assert_db(obs_fader_get_db(fader), 0.0f);
	assert_db(obs_fader_get_mul(fader), 1.0f);

	obs_fader_destroy(fader);
}

static void iec_breakpoints_test(void **state)
{
	obs_fader_t *fader = obs_fader_create(OBS_FADER_IEC);
	static const float def[7] = {1.0f, 0.75f, 0.5f, 0.3f, 0.15f, 0.075f, 0.025f};
	static const float db[7] = {0.0f, -9.0f, -20.0f, -30.0f, -40.0f, -50.0f, -60.0f};
	int i;

	(void)state;
	for (i = 0; i < 7; i++) {
		assert_true(obs_fader_set_deflection(fader, def[i]));
		assert_db(obs_fader_get_db(fader), db[i]);
		assert_db(obs_fader_get_deflection(fader), def[i]);
	}

	assert_true(obs_fader_set_deflection(fader, 0.0f));
	assert_db(obs_fader_get_db(fader), -INFINITY);
	assert_db(obs_fader_get_deflection(fader), 0.0f);

	/* characterized, not endorsed: def 0.001 maps to -146.4 dB, and the
	 * reverse map treats anything below -114 dB as deflection 0. */
	assert_true(obs_fader_set_deflection(fader, 0.001f));
	assert_db(obs_fader_get_db(fader), -146.4f);
	assert_db(obs_fader_get_deflection(fader), 0.0f);

	assert_true(obs_fader_set_deflection(fader, 0.01f));
	assert_db(obs_fader_get_db(fader), -114.0f);
	assert_db(obs_fader_get_deflection(fader), 0.01f);

	obs_fader_destroy(fader);
}

static void log_curve_test(void **state)
{
	obs_fader_t *fader = obs_fader_create(OBS_FADER_LOG);

	(void)state;
	assert_true(obs_fader_set_deflection(fader, 0.5f));
	assert_db(obs_fader_get_db(fader), -18.7386f);
	assert_db(obs_fader_get_deflection(fader), 0.5f);

	assert_true(obs_fader_set_deflection(fader, 0.25f));
	assert_db(obs_fader_get_db(fader), -44.2329f);

	/* def >= 1 is already 0 dB, so set_deflection does not report a clamp. */
	assert_true(obs_fader_set_deflection(fader, 1.5f));
	assert_db(obs_fader_get_db(fader), 0.0f);
	assert_db(obs_fader_get_deflection(fader), 1.0f);

	assert_true(obs_fader_set_db(fader, -96.0f));
	assert_db(obs_fader_get_db(fader), -96.0f);
	assert_db(obs_fader_get_deflection(fader), 0.0f);

	/* characterized, not endorsed: below -96 the stored value is -infinity,
	 * not the -96 floor. set_mul(0) takes the same path. */
	assert_false(obs_fader_set_db(fader, -100.0f));
	assert_db(obs_fader_get_db(fader), -INFINITY);
	assert_db(obs_fader_get_deflection(fader), 0.0f);
	assert_db(obs_fader_get_mul(fader), 0.0f);

	assert_false(obs_fader_set_mul(fader, 0.0f));
	assert_db(obs_fader_get_db(fader), -INFINITY);
	assert_db(obs_fader_get_mul(fader), 0.0f);

	obs_fader_destroy(fader);
}

struct call_log {
	int seq[8];
	int n;
};

struct fader_note {
	int id;
	int calls;
	float db;
	struct call_log *log;
};

static void on_fader(void *param, float db)
{
	struct fader_note *note = param;

	if (note->log && note->log->n < 8)
		note->log->seq[note->log->n++] = note->id;
	note->db = db;
	note->calls++;
}

static void callback_order_and_source_test(void **state)
{
	obs_fader_t *fader = obs_fader_create(OBS_FADER_CUBIC);
	obs_source_t *first = make_source();
	obs_source_t *second = make_source();
	struct call_log log = {0};
	struct fader_note a = {.id = 1, .log = &log};
	struct fader_note b = {.id = 2, .log = &log};
	int before;

	(void)state;
	assert_false(obs_fader_attach_source(fader, NULL));
	assert_true(obs_fader_attach_source(fader, first));
	assert_db(obs_fader_get_db(fader), 0.0f);

	before = null_fader_logs;
	obs_fader_add_callback(NULL, on_fader, &a);
	assert_int_equal(null_fader_logs, before + 1);

	obs_fader_add_callback(fader, on_fader, &a);
	obs_fader_add_callback(fader, on_fader, &b);

	/* Setting through the fader swallows the volume signal it causes. */
	assert_true(obs_fader_set_db(fader, -12.0f));
	assert_int_equal(a.calls, 0);
	assert_int_equal(b.calls, 0);
	assert_db(obs_fader_get_db(fader), -12.0f);
	assert_float_equal(obs_source_get_volume(first), obs_db_to_mul(-12.0f), 0.0001);

	obs_source_set_volume(first, 0.5f);
	assert_int_equal(a.calls, 1);
	assert_int_equal(b.calls, 1);
	/* Walk is last-added first. */
	assert_int_equal(log.n, 2);
	assert_int_equal(log.seq[0], 2);
	assert_int_equal(log.seq[1], 1);
	assert_db(b.db, obs_mul_to_db(0.5f));
	assert_db(obs_fader_get_db(fader), obs_mul_to_db(0.5f));

	log.n = 0;
	obs_fader_remove_callback(fader, on_fader, &b);
	obs_source_set_volume(first, 0.25f);
	assert_int_equal(b.calls, 1);
	assert_int_equal(a.calls, 2);
	assert_int_equal(log.n, 1);
	assert_int_equal(log.seq[0], 1);
	assert_db(obs_fader_get_db(fader), obs_mul_to_db(0.25f));

	/* The first source is detached. Its later volume change is ignored. */
	assert_true(obs_fader_attach_source(fader, second));
	assert_db(obs_fader_get_db(fader), 0.0f);
	a.calls = 0;
	obs_source_set_volume(first, 0.1f);
	assert_int_equal(a.calls, 0);
	log.n = 0;
	obs_source_set_volume(second, 0.8f);
	assert_int_equal(a.calls, 1);
	assert_int_equal(log.n, 1);
	assert_int_equal(log.seq[0], 1);
	assert_db(a.db, obs_mul_to_db(0.8f));

	/* Detach leaves the last dB and ignores later volume changes. */
	obs_fader_detach_source(fader);
	log.n = 0;
	a.calls = 0;
	obs_source_set_volume(second, 0.4f);
	assert_int_equal(a.calls, 0);
	assert_int_equal(log.n, 0);
	assert_db(obs_fader_get_db(fader), obs_mul_to_db(0.8f));

	obs_source_release(first);
	obs_source_release(second);
	/* Detach runs from the deferred destroy signal. The last dB stays. */
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	assert_db(obs_fader_get_db(fader), obs_mul_to_db(0.8f));
	assert_true(obs_fader_set_db(fader, -3.0f));
	assert_db(obs_fader_get_db(fader), -3.0f);

	obs_fader_destroy(fader);
}

/* characterized, not endorsed: set_db arms ignore_next_signal even with no
 * source attached. The next external volume change is dropped, and cur_db
 * stays at the value attach copied. */
static void ignore_sticks_across_attach_test(void **state)
{
	obs_fader_t *fader = obs_fader_create(OBS_FADER_CUBIC);
	obs_source_t *source = make_source();
	struct fader_note note = {.id = 1};

	(void)state;
	assert_true(obs_fader_set_db(fader, -10.0f));
	obs_fader_add_callback(fader, on_fader, &note);
	assert_true(obs_fader_attach_source(fader, source));
	assert_db(obs_fader_get_db(fader), 0.0f);

	obs_source_set_volume(source, 0.5f);
	assert_int_equal(note.calls, 0);
	assert_db(obs_fader_get_db(fader), 0.0f);

	obs_source_set_volume(source, 0.25f);
	assert_int_equal(note.calls, 1);
	assert_db(obs_fader_get_db(fader), obs_mul_to_db(0.25f));

	obs_fader_destroy(fader);
	obs_source_release(source);
}

struct meter_note {
	int id;
	int calls;
	struct call_log *log;
	float magnitude[MAX_AUDIO_CHANNELS];
	float peak[MAX_AUDIO_CHANNELS];
	float input_peak[MAX_AUDIO_CHANNELS];
};

static void on_meter(void *param, const float magnitude[MAX_AUDIO_CHANNELS], const float peak[MAX_AUDIO_CHANNELS],
		     const float input_peak[MAX_AUDIO_CHANNELS])
{
	struct meter_note *note = param;

	if (note->log && note->log->n < 8)
		note->log->seq[note->log->n++] = note->id;
	memcpy(note->magnitude, magnitude, sizeof(note->magnitude));
	memcpy(note->peak, peak, sizeof(note->peak));
	memcpy(note->input_peak, input_peak, sizeof(note->input_peak));
	note->calls++;
}

static void output_stereo(obs_source_t *source, float left, float right, uint32_t frames)
{
	float lbuf[32];
	float rbuf[32];
	struct obs_source_audio audio;
	uint32_t i;

	assert_true(frames <= 32);
	for (i = 0; i < frames; i++) {
		lbuf[i] = left;
		rbuf[i] = right;
	}
	memset(&audio, 0, sizeof(audio));
	audio.data[0] = (uint8_t *)lbuf;
	audio.data[1] = (uint8_t *)rbuf;
	audio.frames = frames;
	audio.speakers = SPEAKERS_STEREO;
	audio.format = AUDIO_FORMAT_FLOAT_PLANAR;
	audio.samples_per_sec = RATE;
	audio.timestamp = os_gettime_ns();
	obs_source_output_audio(source, &audio);
}

static void volmeter_levels_test(void **state)
{
	obs_volmeter_t *cubic = obs_volmeter_create(OBS_FADER_CUBIC);
	obs_volmeter_t *logf = obs_volmeter_create(OBS_FADER_LOG);
	obs_source_t *source = make_source();
	struct meter_note a = {0};
	struct meter_note b = {0};
	int channel;

	(void)state;
	assert_int_equal(obs_volmeter_get_nr_channels(cubic), 0);
	assert_false(obs_volmeter_attach_source(cubic, NULL));
	assert_true(obs_volmeter_attach_source(cubic, source));
	assert_true(obs_volmeter_attach_source(logf, source));
	/* No packet yet, so sample_info.speakers is still unknown. */
	assert_int_equal(obs_volmeter_get_nr_channels(cubic), 0);

	obs_volmeter_add_callback(cubic, on_meter, &a);
	obs_volmeter_add_callback(logf, on_meter, &b);
	output_stereo(source, 0.5f, 0.25f, TONE_FRAMES);

	assert_int_equal(a.calls, 1);
	assert_int_equal(b.calls, 1);
	assert_int_equal(obs_volmeter_get_nr_channels(cubic), 2);
	/* characterized, not endorsed: the fader type is stored and not used. */
	for (channel = 0; channel < MAX_AUDIO_CHANNELS; channel++) {
		assert_db(a.magnitude[channel], b.magnitude[channel]);
		assert_db(a.peak[channel], b.peak[channel]);
		assert_db(a.input_peak[channel], b.input_peak[channel]);
	}
	assert_db(a.magnitude[0], obs_mul_to_db(0.5f));
	assert_db(a.magnitude[1], obs_mul_to_db(0.25f));
	assert_db(a.peak[0], obs_mul_to_db(0.5f));
	assert_db(a.peak[1], obs_mul_to_db(0.25f));
	assert_db(a.input_peak[0], obs_mul_to_db(0.5f));
	for (channel = 2; channel < MAX_AUDIO_CHANNELS; channel++) {
		assert_db(a.magnitude[channel], -INFINITY);
		assert_db(a.peak[channel], -INFINITY);
	}

	obs_source_set_volume(source, 0.5f);
	output_stereo(source, 0.5f, 0.25f, TONE_FRAMES);
	assert_int_equal(a.calls, 2);
	assert_db(a.magnitude[0], obs_mul_to_db(0.25f));
	assert_db(a.peak[0], obs_mul_to_db(0.25f));
	/* input peak is the sample peak, before volume. */
	assert_db(a.input_peak[0], obs_mul_to_db(0.5f));
	assert_db(a.input_peak[1], obs_mul_to_db(0.25f));

	obs_volmeter_remove_callback(cubic, on_meter, &a);
	output_stereo(source, 0.5f, 0.25f, TONE_FRAMES);
	assert_int_equal(a.calls, 2);
	assert_int_equal(b.calls, 3);

	obs_source_release(source);
	/* Detach runs from the deferred destroy signal. */
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	assert_int_equal(obs_volmeter_get_nr_channels(cubic), 0);
	obs_volmeter_destroy(cubic);
	obs_volmeter_destroy(logf);
}

/* characterized, not endorsed: user mute does not zero the meter. The meter
 * zeros only when the muted flag and obs_source_muted disagree, which a
 * disabled source does. */
static void mute_and_disable_test(void **state)
{
	obs_volmeter_t *meter = obs_volmeter_create(OBS_FADER_CUBIC);
	obs_source_t *source = make_source();
	struct meter_note note = {0};

	(void)state;
	assert_true(obs_volmeter_attach_source(meter, source));
	obs_volmeter_add_callback(meter, on_meter, &note);

	obs_source_set_muted(source, true);
	output_stereo(source, 0.5f, 0.5f, TONE_FRAMES);
	assert_int_equal(note.calls, 1);
	assert_db(note.magnitude[0], obs_mul_to_db(0.5f));

	obs_source_set_muted(source, false);
	obs_source_set_enabled(source, false);
	output_stereo(source, 0.5f, 0.5f, TONE_FRAMES);
	assert_int_equal(note.calls, 2);
	assert_db(note.magnitude[0], -INFINITY);
	assert_db(note.peak[0], -INFINITY);
	assert_db(note.input_peak[0], obs_mul_to_db(0.5f));

	obs_volmeter_destroy(meter);
	obs_source_release(source);
}

/* characterized, not endorsed: a packet shorter than 4 samples does not
 * enter the peak loop. Magnitude still uses every sample. */
static void short_packet_peak_test(void **state)
{
	obs_volmeter_t *meter = obs_volmeter_create(OBS_FADER_CUBIC);
	obs_source_t *source = make_source();
	struct meter_note note = {0};

	(void)state;
	assert_true(obs_volmeter_attach_source(meter, source));
	obs_volmeter_add_callback(meter, on_meter, &note);
	output_stereo(source, 1.0f, 1.0f, 2);

	assert_int_equal(note.calls, 1);
	assert_db(note.magnitude[0], 0.0f);
	assert_db(note.peak[0], -INFINITY);
	assert_db(note.input_peak[0], -INFINITY);

	/* Those two samples were shifted into the peak history. */
	output_stereo(source, 0.0f, 0.0f, TONE_FRAMES);
	assert_int_equal(note.calls, 2);
	assert_db(note.magnitude[0], -INFINITY);
	assert_db(note.peak[0], 0.0f);
	assert_db(note.input_peak[0], 0.0f);

	/* The silence packet replaces that history. */
	output_stereo(source, 0.0f, 0.0f, TONE_FRAMES);
	assert_int_equal(note.calls, 3);
	assert_db(note.peak[0], -INFINITY);
	assert_db(note.input_peak[0], -INFINITY);

	obs_volmeter_destroy(meter);
	obs_source_release(source);
}

static void output_impulse(obs_source_t *source)
{
	float left[8] = {0.0f, 1.0f, 0.0f, -1.0f, 0.0f, 1.0f, 0.0f, -1.0f};
	float right[8] = {0};
	struct obs_source_audio audio;

	memset(&audio, 0, sizeof(audio));
	audio.data[0] = (uint8_t *)left;
	audio.data[1] = (uint8_t *)right;
	audio.frames = 8;
	audio.speakers = SPEAKERS_STEREO;
	audio.format = AUDIO_FORMAT_FLOAT_PLANAR;
	audio.samples_per_sec = RATE;
	audio.timestamp = os_gettime_ns();
	obs_source_output_audio(source, &audio);
}

static void sample_and_true_peak_test(void **state)
{
	obs_volmeter_t *sample = obs_volmeter_create(OBS_FADER_CUBIC);
	obs_volmeter_t *truth = obs_volmeter_create(OBS_FADER_CUBIC);
	obs_source_t *source = make_source();
	struct meter_note sample_note = {0};
	struct meter_note true_note = {0};

	(void)state;
	obs_volmeter_set_peak_meter_type(sample, SAMPLE_PEAK_METER);
	obs_volmeter_set_peak_meter_type(truth, TRUE_PEAK_METER);
	assert_true(obs_volmeter_attach_source(sample, source));
	assert_true(obs_volmeter_attach_source(truth, source));
	obs_volmeter_add_callback(sample, on_meter, &sample_note);
	obs_volmeter_add_callback(truth, on_meter, &true_note);

	output_impulse(source);
	assert_int_equal(sample_note.calls, 1);
	assert_int_equal(true_note.calls, 1);
	assert_db(sample_note.peak[0], 0.0f);
	assert_db(sample_note.input_peak[0], 0.0f);
	assert_db(sample_note.peak[1], -INFINITY);
	/* Magnitude is RMS. Four of the eight samples are ±1, so this is -3.0103 dB. */
	assert_db(sample_note.magnitude[0], obs_mul_to_db(sqrtf(0.5f)));
	assert_db(true_note.magnitude[0], obs_mul_to_db(sqrtf(0.5f)));
	assert_db(sample_note.magnitude[1], -INFINITY);
	/* characterized, not endorsed: this buffer overshoots 0 dB.
	 * The C meter reported 0x1.57fbf4p-2 for it. */
	assert_db(true_note.peak[0], 0x1.57fbf4p-2f);
	assert_db(true_note.input_peak[0], 0x1.57fbf4p-2f);
	assert_db(true_note.peak[1], -INFINITY);

	obs_volmeter_destroy(sample);
	obs_volmeter_destroy(truth);
	obs_source_release(source);
}

/* Last callback added runs first, same walk as the fader. */
static void meter_callback_order_test(void **state)
{
	obs_volmeter_t *meter = obs_volmeter_create(OBS_FADER_CUBIC);
	obs_source_t *source = make_source();
	struct call_log log = {0};
	struct meter_note a = {.id = 1, .log = &log};
	struct meter_note b = {.id = 2, .log = &log};

	(void)state;
	assert_true(obs_volmeter_attach_source(meter, source));
	obs_volmeter_add_callback(meter, on_meter, &a);
	obs_volmeter_add_callback(meter, on_meter, &b);
	output_stereo(source, 0.5f, 0.5f, TONE_FRAMES);

	assert_int_equal(log.n, 2);
	assert_int_equal(log.seq[0], 2);
	assert_int_equal(log.seq[1], 1);

	obs_volmeter_destroy(meter);
	obs_source_release(source);
}

static void meter_reattach_and_detach_test(void **state)
{
	obs_volmeter_t *meter = obs_volmeter_create(OBS_FADER_CUBIC);
	obs_source_t *first = make_source();
	obs_source_t *second = make_source();
	struct meter_note note = {0};

	(void)state;
	assert_true(obs_volmeter_attach_source(meter, first));
	obs_volmeter_add_callback(meter, on_meter, &note);
	obs_source_set_volume(first, 0.5f);
	output_stereo(first, 1.0f, 1.0f, TONE_FRAMES);
	assert_int_equal(note.calls, 1);
	assert_db(note.magnitude[0], obs_mul_to_db(0.5f));

	assert_true(obs_volmeter_attach_source(meter, second));
	output_stereo(first, 1.0f, 1.0f, TONE_FRAMES);
	assert_int_equal(note.calls, 1);

	output_stereo(second, 1.0f, 1.0f, TONE_FRAMES);
	assert_int_equal(note.calls, 2);
	/* Attach copies the new source's volume, which is unity. */
	assert_db(note.magnitude[0], 0.0f);

	obs_volmeter_detach_source(meter);
	output_stereo(second, 1.0f, 1.0f, TONE_FRAMES);
	assert_int_equal(note.calls, 2);

	obs_volmeter_destroy(meter);
	obs_source_release(first);
	obs_source_release(second);
}

/* 7.1 into a stereo mix is clamped to the mix. */
static void channel_count_clamps_test(void **state)
{
	obs_volmeter_t *meter = obs_volmeter_create(OBS_FADER_CUBIC);
	obs_source_t *source = make_source();
	float planes[8][8];
	struct obs_source_audio audio;
	int i;

	(void)state;
	assert_true(obs_volmeter_attach_source(meter, source));
	memset(planes, 0, sizeof(planes));
	memset(&audio, 0, sizeof(audio));
	for (i = 0; i < 8; i++) {
		planes[i][0] = 0.2f;
		audio.data[i] = (uint8_t *)planes[i];
	}
	audio.frames = 8;
	audio.speakers = SPEAKERS_7POINT1;
	audio.format = AUDIO_FORMAT_FLOAT_PLANAR;
	audio.samples_per_sec = RATE;
	audio.timestamp = os_gettime_ns();
	obs_source_output_audio(source, &audio);

	assert_int_equal(obs_volmeter_get_nr_channels(meter), 2);

	obs_volmeter_destroy(meter);
	obs_source_release(source);
}

int main(void)
{
	const struct CMUnitTest curves[] = {
		cmocka_unit_test(null_pointers_test),   cmocka_unit_test(fresh_fader_is_unity_test),
		cmocka_unit_test(bad_type_test),        cmocka_unit_test(cubic_curve_test),
		cmocka_unit_test(iec_breakpoints_test), cmocka_unit_test(log_curve_test),
	};
	const struct CMUnitTest with_source[] = {
		cmocka_unit_test_setup_teardown(callback_order_and_source_test, source_setup, source_teardown),
		cmocka_unit_test_setup_teardown(ignore_sticks_across_attach_test, source_setup, source_teardown),
		cmocka_unit_test_setup_teardown(volmeter_levels_test, source_setup, source_teardown),
		cmocka_unit_test_setup_teardown(mute_and_disable_test, source_setup, source_teardown),
		cmocka_unit_test_setup_teardown(short_packet_peak_test, source_setup, source_teardown),
		cmocka_unit_test_setup_teardown(sample_and_true_peak_test, source_setup, source_teardown),
		cmocka_unit_test_setup_teardown(meter_callback_order_test, source_setup, source_teardown),
		cmocka_unit_test_setup_teardown(meter_reattach_and_detach_test, source_setup, source_teardown),
		cmocka_unit_test_setup_teardown(channel_count_clamps_test, source_setup, source_teardown),
	};
	int failed = 0;

	failed += cmocka_run_group_tests(curves, NULL, NULL);
	failed += cmocka_run_group_tests(with_source, NULL, NULL);
	return failed;
}
