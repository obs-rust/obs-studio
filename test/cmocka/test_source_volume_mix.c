/*
 * A source's volume as the mix applies it (#2108): out of range, from the
 * setter or from a loaded scene collection, it must not break the mix.
 *
 * libobs multiplies every sample of a source by its volume. A NaN volume
 * made every sample NaN, and NaN spreads through the sum into every mix the
 * source is in; an infinite one made the silent samples NaN (inf * 0) and
 * the others infinite; a negative one inverted the phase. Now NaN is the
 * default, 1, a negative volume is 0 and +inf is 20 (+26 dB), the most the
 * Advanced Audio Properties and obs-websocket set; finite volumes are kept,
 * so 100 (+40 dB) still amplifies 100 times.
 *
 * A thread delivers 10 ms packets stamped with the system clock, like a live
 * capture: a quiet constant on the left channel and silence on the right.
 * The source is put on the program, and the mixed audio libobs hands to
 * outputs (obs_add_raw_audio_callback) must be finite and carry the left
 * channel at the mapped volume. Both mixes are checked: the clamped one most
 * outputs get, which turns NaN into silence (so a NaN volume silenced the
 * whole track), and the unclamped one that outputs allowing clipping get,
 * where NaN and inf come through. test_source_audio_ranges checks the getter,
 * the "volume" signal and the volume queued for the mix bit for bit. Mixing
 * needs core video, so the cases run under xvfb-run and skip without a
 * display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define SOURCE_ID "test_source_volume_mix_capture"

#define W 64
#define H 64
#define RATE 48000
#define MSEC 1000000ULL
#define PACKET_FRAMES 480 /* 10 ms */
#define PACKET_NS (10 * MSEC)
/* small enough that 100 times it is not clipped */
#define LEVEL 0.004f
#define TOLERANCE 1e-6f
/* frames of the source at its volume to see before a case passes */
#define WANT_FRAMES 4096
#define WAIT_MS 4000
/* how long a silent case listens */
#define LISTEN_MS 500

static bool obs_up;
static int source_count;

/* ------------------------------------------------------------------------ */
/* the capture */

struct capture {
	obs_source_t *source;
	pthread_t thread;
	bool thread_started;
	volatile bool stop;
};

static void *capture_thread(void *param)
{
	struct capture *c = param;
	float left[PACKET_FRAMES];
	static float right[PACKET_FRAMES];
	for (size_t i = 0; i < PACKET_FRAMES; i++)
		left[i] = LEVEL;

	/* the first packet comes after the setter or the load has set the
	 * volume, so none of it is mixed at the default */
	os_sleep_ms(50);
	uint64_t t0 = os_gettime_ns();
	for (uint64_t n = 0; !c->stop; n++) {
		uint64_t ts = t0 + n * PACKET_NS;

		/* a period is delivered when it has been captured */
		os_sleepto_ns(ts + PACKET_NS);

		struct obs_source_audio audio = {
			.data = {(const uint8_t *)left, (const uint8_t *)right},
			.frames = PACKET_FRAMES,
			.speakers = SPEAKERS_STEREO,
			.format = AUDIO_FORMAT_FLOAT_PLANAR,
			.samples_per_sec = RATE,
			.timestamp = ts,
		};
		obs_source_output_audio(c->source, &audio);
	}
	return NULL;
}

static const char *capture_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "volume mix capture";
}

/* Like a live capture, it starts capturing when it is created, before a
 * loaded scene collection sets its volume; its first packet is 50 ms later. */
static void *capture_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	struct capture *c = bzalloc(sizeof(*c));
	c->source = source;
	if (pthread_create(&c->thread, NULL, capture_thread, c) == 0)
		c->thread_started = true;
	else
		fprintf(stderr, "could not start the capture thread\n");
	return c;
}

static void capture_destroy(void *data)
{
	struct capture *c = data;
	if (c->thread_started) {
		c->stop = true;
		pthread_join(c->thread, NULL);
	}
	bfree(c);
}

static struct obs_source_info capture_info = {
	.id = SOURCE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = capture_name,
	.create = capture_create,
	.destroy = capture_destroy,
};

/* ------------------------------------------------------------------------ */
/* the program audio */

struct program {
	pthread_mutex_t mutex;
	float expected;
	size_t matching;  /* left at the expected level, right silent */
	size_t nonfinite; /* NaN or infinite samples, either channel */
	size_t other;     /* finite, but neither silence nor the source */
	float first_other;
};

static struct program clamped;
static struct program unclamped;
static struct program *const programs[] = {&clamped, &unclamped};
#define NUM_PROGRAMS (sizeof(programs) / sizeof(programs[0]))

static void raw_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	UNUSED_PARAMETER(mix_idx);
	struct program *p = param;
	const float *left = (const float *)data->data[0];
	const float *right = (const float *)data->data[1];

	pthread_mutex_lock(&p->mutex);
	for (uint32_t i = 0; i < data->frames; i++) {
		if (!isfinite(left[i]) || !isfinite(right[i])) {
			p->nonfinite++;
		} else if (right[i] != 0.0f) {
			if (!p->other++)
				p->first_other = right[i];
		} else if (fabsf(left[i] - p->expected) <= TOLERANCE) {
			p->matching++;
		} else if (left[i] != 0.0f) {
			/* silence is before the source's audio arrives */
			if (!p->other++)
				p->first_other = left[i];
		}
	}
	pthread_mutex_unlock(&p->mutex);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
		return 0;
	}

	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = W,
		.base_height = H,
		.output_width = W,
		.output_height = H,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed\n");
		obs_shutdown();
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "skipped: obs_reset_audio failed\n");
		obs_shutdown();
		return 0;
	}

	obs_register_source(&capture_info);
	pthread_mutex_init(&clamped.mutex, NULL);
	pthread_mutex_init(&unclamped.mutex, NULL);
	const struct audio_convert_info allow_clipping = {.allow_clipping = true};
	obs_add_raw_audio_callback(0, NULL, raw_audio, &clamped);
	obs_add_raw_audio_callback(0, &allow_clipping, raw_audio, &unclamped);

	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		obs_remove_raw_audio_callback(0, raw_audio, &clamped);
		obs_remove_raw_audio_callback(0, raw_audio, &unclamped);
		obs_shutdown();
		pthread_mutex_destroy(&clamped.mutex);
		pthread_mutex_destroy(&unclamped.mutex);
		obs_up = false;
	}
	return 0;
}

/* ------------------------------------------------------------------------ */
/* measuring */

/* the source under test, cleaned up after each case even if it failed */
static obs_source_t *live;

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!live)
		return 0;
	obs_set_output_source(0, NULL);
	obs_source_remove(live);
	obs_source_release(live);
	live = NULL;
	return 0;
}

static obs_source_t *create_source(void)
{
	char name[64];
	snprintf(name, sizeof(name), "volume mix %d", source_count++);
	live = obs_source_create(SOURCE_ID, name, NULL, NULL);
	assert_non_null(live);
	return live;
}

static obs_source_t *load_source(double volume)
{
	char name[64];
	snprintf(name, sizeof(name), "volume mix %d", source_count++);
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "id", SOURCE_ID);
	obs_data_set_string(data, "name", name);
	obs_data_set_double(data, "volume", volume);
	live = obs_load_source(data);
	obs_data_release(data);
	assert_non_null(live);
	return live;
}

static void reset_program(struct program *p, float expected)
{
	pthread_mutex_lock(&p->mutex);
	p->expected = expected;
	p->matching = 0;
	p->nonfinite = 0;
	p->other = 0;
	pthread_mutex_unlock(&p->mutex);
}

static void read_program(struct program *p, struct program *copy)
{
	pthread_mutex_lock(&p->mutex);
	copy->expected = p->expected;
	copy->matching = p->matching;
	copy->nonfinite = p->nonfinite;
	copy->other = p->other;
	copy->first_other = p->first_other;
	pthread_mutex_unlock(&p->mutex);
}

/* Puts the source on the program and listens: both mixes must stay finite
 * and carry the source's left channel at LEVEL * mul, or stay silent at 0.
 * Reports every case before the test fails. */
static bool expect_mix(obs_source_t *source, const char *what, double in, float mul)
{
	static const char *const names[] = {"clamped", "unclamped"};
	struct program seen[NUM_PROGRAMS] = {0};

	/* what is left of the previous case leaves the mix first */
	os_sleep_ms(100);
	for (size_t i = 0; i < NUM_PROGRAMS; i++)
		reset_program(programs[i], LEVEL * mul);

	obs_set_output_source(0, source);

	int limit = mul == 0.0f ? LISTEN_MS : WAIT_MS;
	for (int waited = 0; waited < limit; waited += 20) {
		os_sleep_ms(20);
		bool done = mul != 0.0f;
		bool bad = false;
		for (size_t i = 0; i < NUM_PROGRAMS; i++) {
			read_program(programs[i], &seen[i]);
			bad = bad || seen[i].nonfinite || seen[i].other;
			done = done && seen[i].matching >= WANT_FRAMES;
		}
		if (bad || done)
			break;
	}
	obs_set_output_source(0, NULL);

	bool failed = false;
	for (size_t i = 0; i < NUM_PROGRAMS; i++) {
		const struct program *p = &seen[i];
		fprintf(stderr, "%s %g, %s mix: volume %a, %zu frames at %g, %zu not finite, %zu other\n", what, in,
			names[i], (double)obs_source_get_volume(source), p->matching, (double)(LEVEL * mul),
			p->nonfinite, p->other);
		if (p->nonfinite) {
			fprintf(stderr, "%s %g: %zu %s mixed samples are NaN or infinite\n", what, in, p->nonfinite,
				names[i]);
			failed = true;
		}
		if (p->other) {
			fprintf(stderr, "%s %g: %s mixed sample %g, expected %g or silence\n", what, in, names[i],
				(double)p->first_other, (double)(LEVEL * mul));
			failed = true;
		}
		if (mul != 0.0f && p->matching < WANT_FRAMES) {
			fprintf(stderr, "%s %g: only %zu of %d %s frames carry the source at its volume\n", what, in,
				p->matching, WANT_FRAMES, names[i]);
			failed = true;
		}
	}
	return !failed;
}

/* ------------------------------------------------------------------------ */
/* cases */

struct mix_case {
	double in;
	float mul;
};

/* -1e300 and 1e300 only through a scene collection, which stores doubles */
static const struct mix_case out_of_range[] = {
	{NAN, 1.0f},
	{-1.0, 0.0f},
	{INFINITY, 20.0f},
	{-INFINITY, 0.0f},
};

/* the mix as before: 0 silences the source, 1 passes it, 100 amplifies it */
static const struct mix_case valid[] = {
	{0.0, 0.0f},
	{1.0, 1.0f},
	{100.0, 100.0f},
};

static void setter_out_of_range_mixes_finite(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	bool ok = true;
	for (size_t i = 0; i < sizeof(out_of_range) / sizeof(out_of_range[0]); i++) {
		obs_source_t *source = create_source();
		obs_source_set_volume(source, (float)out_of_range[i].in);
		ok &= expect_mix(source, "set", out_of_range[i].in, out_of_range[i].mul);
		case_teardown(NULL);
	}
	if (!ok)
		fail();
}

static void setter_valid_mixes_as_before(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	bool ok = true;
	for (size_t i = 0; i < sizeof(valid) / sizeof(valid[0]); i++) {
		obs_source_t *source = create_source();
		obs_source_set_volume(source, (float)valid[i].in);
		ok &= expect_mix(source, "set", valid[i].in, valid[i].mul);
		case_teardown(NULL);
	}
	if (!ok)
		fail();
}

static void loaded_out_of_range_mixes_finite(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	bool ok = true;
	for (size_t i = 0; i < sizeof(out_of_range) / sizeof(out_of_range[0]); i++) {
		obs_source_t *source = load_source(out_of_range[i].in);
		ok &= expect_mix(source, "loaded", out_of_range[i].in, out_of_range[i].mul);
		case_teardown(NULL);
	}

	/* beyond float's range */
	obs_source_t *source = load_source(1e300);
	ok &= expect_mix(source, "loaded", 1e300, 20.0f);
	case_teardown(NULL);
	source = load_source(-1e300);
	ok &= expect_mix(source, "loaded", -1e300, 0.0f);
	case_teardown(NULL);
	if (!ok)
		fail();
}

static void loaded_valid_mixes_as_before(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	bool ok = true;
	for (size_t i = 0; i < sizeof(valid) / sizeof(valid[0]); i++) {
		obs_source_t *source = load_source(valid[i].in);
		ok &= expect_mix(source, "loaded", valid[i].in, valid[i].mul);
		case_teardown(NULL);
	}
	if (!ok)
		fail();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(setter_out_of_range_mixes_finite, case_teardown),
		cmocka_unit_test_teardown(setter_valid_mixes_as_before, case_teardown),
		cmocka_unit_test_teardown(loaded_out_of_range_mixes_finite, case_teardown),
		cmocka_unit_test_teardown(loaded_valid_mixes_as_before, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
