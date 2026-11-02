/*
 * A transition mixes each audio block with one copy of its clock
 * (obs-rust/obs-studio#3499).
 *
 * obs_transition_audio_render() reads the clock under the transition's lock
 * to decide whether the audio stops, then drops the lock and mixes. The mix
 * must use the clock it read under the lock: a manual tick, a start, a mode
 * change or a load can write the clock while the block is mixing, and one
 * block (or two channels of it) must not be mixed at two positions.
 *
 * The test runs a manual transition between two constant tones at 0.25.
 * On one block the transition's first mix call has another thread move the
 * manual value back to 0 (obs_transition_set_manual_time(), as a T-bar let
 * go at the start does) and waits for it, so the write lands while the
 * block is mixing. Every position the mix callbacks get for that block
 * must be the same. It needs core video, so it runs (under xvfb-run) on
 * Linux and skips without a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <obs-module.h>
#include <util/platform.h>
#include <util/threading.h>

#define RATE 48000
#define LEVEL 0.5f
#define START_T 0.25f

/* ------------------------------------------------------------------------ */
/* Constant-level audio source                                               */

struct tone {
	obs_source_t *source;
	pthread_t thread;
	os_event_t *stop;
};

static void *tone_thread(void *data)
{
	struct tone *t = data;
	float buf[480];
	for (size_t i = 0; i < 480; i++)
		buf[i] = LEVEL;

	uint64_t next = os_gettime_ns();
	while (os_event_try(t->stop) == EAGAIN) {
		struct obs_source_audio a = {
			.data = {(uint8_t *)buf, (uint8_t *)buf},
			.frames = 480,
			.speakers = SPEAKERS_STEREO,
			.format = AUDIO_FORMAT_FLOAT_PLANAR,
			.samples_per_sec = RATE,
			.timestamp = next,
		};
		obs_source_output_audio(t->source, &a);
		next += 10000000ULL;
		os_sleepto_ns(next);
	}
	return NULL;
}

static const char *tone_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "tone";
}

static void *tone_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	struct tone *t = bzalloc(sizeof(*t));
	t->source = source;
	os_event_init(&t->stop, OS_EVENT_TYPE_MANUAL);
	pthread_create(&t->thread, NULL, tone_thread, t);
	return t;
}

static void tone_destroy(void *data)
{
	struct tone *t = data;
	os_event_signal(t->stop);
	pthread_join(t->thread, NULL);
	os_event_destroy(t->stop);
	bfree(t);
}

static struct obs_source_info tone_info = {
	.id = "test_tone",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = tone_name,
	.create = tone_create,
	.destroy = tone_destroy,
};

/* ------------------------------------------------------------------------ */
/* Transition that records the positions one block is mixed at               */

static pthread_mutex_t probe_mutex = PTHREAD_MUTEX_INITIALIZER;

/* The position of the last mix call, outside the probed block. */
static float last_t = -1.0f;

/* Set by the test: the next block that mixes is probed. */
static bool armed;
/* The audio thread is inside the probed block's audio render. */
static bool in_probe;
static bool probe_done;
static size_t probe_calls;
/* The other thread's write finished while the probed block was mixing. */
static bool write_in_block;
static float probe_min_t;
static float probe_max_t;

static const char *xfade_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "xfade";
}

static void *xfade_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void xfade_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static void xfade_video_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(effect);
	obs_transition_video_render(data, NULL);
}

/* The other thread: moves the manual value back to 0 when told to. */
static os_event_t *write_go;
static os_event_t *write_done;
static volatile bool writer_stop;

static void *writer_thread(void *data)
{
	obs_source_t *transition = data;

	os_event_wait(write_go);
	if (!writer_stop) {
		obs_transition_set_manual_time(transition, 0.0f);
		os_event_signal(write_done);
	}
	return NULL;
}

static void record_t(float t)
{
	pthread_mutex_lock(&probe_mutex);
	if (!in_probe) {
		last_t = t;
		pthread_mutex_unlock(&probe_mutex);
		return;
	}

	bool first = probe_calls++ == 0;
	if (first) {
		probe_min_t = t;
		probe_max_t = t;
	} else {
		if (t < probe_min_t)
			probe_min_t = t;
		if (t > probe_max_t)
			probe_max_t = t;
	}
	pthread_mutex_unlock(&probe_mutex);

	/* A write from another thread that lands while the block mixes. */
	if (first) {
		os_event_signal(write_go);
		bool done = os_event_timedwait(write_done, 2000) == 0;
		pthread_mutex_lock(&probe_mutex);
		write_in_block = done;
		pthread_mutex_unlock(&probe_mutex);
	}
}

static float mix_a(void *data, float t)
{
	UNUSED_PARAMETER(data);
	record_t(t);
	return 1.0f - t;
}

static float mix_b(void *data, float t)
{
	UNUSED_PARAMETER(data);
	record_t(t);
	return t;
}

static bool xfade_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			       size_t channels, size_t sample_rate)
{
	pthread_mutex_lock(&probe_mutex);
	in_probe = armed && !probe_done;
	if (in_probe)
		probe_calls = 0;
	pthread_mutex_unlock(&probe_mutex);

	bool ret = obs_transition_audio_render(data, ts_out, audio, mixers, channels, sample_rate, mix_a, mix_b);

	pthread_mutex_lock(&probe_mutex);
	/* A block that did not mix is not the probe: try the next one. */
	if (in_probe && probe_calls > 0)
		probe_done = true;
	in_probe = false;
	pthread_mutex_unlock(&probe_mutex);
	return ret;
}

static struct obs_source_info xfade_info = {
	.id = "test_xfade",
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = xfade_name,
	.create = xfade_create,
	.destroy = xfade_destroy,
	.video_render = xfade_video_render,
	.audio_render = xfade_audio_render,
};

/* ------------------------------------------------------------------------ */

/* A listener on mix 0: the audio thread mixes only into mixes that have one. */
static void sink(void *param, size_t mix_idx, struct audio_data *data)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(mix_idx);
	UNUSED_PARAMETER(data);
}

static bool init_core(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return false;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS)
		return false;

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai))
		return false;

	obs_register_source(&tone_info);
	obs_register_source(&xfade_info);
	return true;
}

static bool core_ready;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	const char *display = getenv("DISPLAY");
	if (display && *display)
		core_ready = init_core();
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

/* Waits up to ten seconds for the audio thread to mix at START_T. */
static bool wait_for_start_t(void)
{
	for (int i = 0; i < 1000; i++) {
		pthread_mutex_lock(&probe_mutex);
		bool ready = last_t == START_T;
		pthread_mutex_unlock(&probe_mutex);
		if (ready)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

/* Waits up to ten seconds for the probed block. */
static bool wait_for_probe(void)
{
	for (int i = 0; i < 1000; i++) {
		pthread_mutex_lock(&probe_mutex);
		bool done = probe_done;
		pthread_mutex_unlock(&probe_mutex);
		if (done)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

static void block_mixes_with_one_clock(void **state)
{
	UNUSED_PARAMETER(state);
	if (!core_ready)
		skip();

	obs_source_t *outgoing = obs_source_create("test_tone", "outgoing", NULL, NULL);
	obs_source_t *incoming = obs_source_create("test_tone", "incoming", NULL, NULL);
	/* Not private: the video thread ticks only listed sources, and the tick
	 * moves the manual value. */
	obs_source_t *xfade = obs_source_create("test_xfade", "xfade", NULL, NULL);

	obs_transition_set(xfade, outgoing);
	obs_set_output_source(0, xfade);
	assert_true(audio_output_connect(obs_get_audio(), 0, NULL, sink, NULL));

	obs_transition_set_manual_torque(xfade, 0.0f, 0.0f);
	obs_transition_start(xfade, OBS_TRANSITION_MODE_MANUAL, 0, incoming);
	obs_transition_set_manual_time(xfade, START_T);

	pthread_t writer;
	os_event_init(&write_go, OS_EVENT_TYPE_AUTO);
	os_event_init(&write_done, OS_EVENT_TYPE_AUTO);
	pthread_create(&writer, NULL, writer_thread, xfade);

	bool started = wait_for_start_t();
	bool probed = false;
	if (started) {
		pthread_mutex_lock(&probe_mutex);
		armed = true;
		pthread_mutex_unlock(&probe_mutex);
		probed = wait_for_probe();
	}

	audio_output_disconnect(obs_get_audio(), 0, sink, NULL);
	obs_set_output_source(0, NULL);
	writer_stop = !probed;
	os_event_signal(write_go);
	pthread_join(writer, NULL);
	os_event_destroy(write_go);
	os_event_destroy(write_done);
	obs_source_remove(xfade);
	obs_source_release(xfade);
	obs_source_remove(outgoing);
	obs_source_release(outgoing);
	obs_source_remove(incoming);
	obs_source_release(incoming);

	assert_true(started);
	assert_true(probed);

	pthread_mutex_lock(&probe_mutex);
	size_t calls = probe_calls;
	bool in_block = write_in_block;
	float min_t = probe_min_t;
	float max_t = probe_max_t;
	pthread_mutex_unlock(&probe_mutex);

	fprintf(stderr, "probed block: %zu mix calls, write %s, positions %.3f to %.3f\n", calls,
		in_block ? "inside the block" : "not finished in time", (double)min_t, (double)max_t);
	assert_true(in_block);
	assert_true(min_t == START_T);
	assert_true(max_t == START_T);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(block_mixes_with_one_clock),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
