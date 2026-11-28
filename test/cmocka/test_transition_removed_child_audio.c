/*
 * A transition's audio must not come from a child that was removed
 * (obs-rust/obs-studio#2185).
 *
 * obs_transition_audio_render() drops removed children before it computes
 * the tick's timestamp, then reads the children again after a possible
 * audio stop. That second read must drop removed children too: after the
 * stop the removed destination becomes source A, and counting it would hand
 * its timestamp and audio to the mix.
 *
 * The test removes the incoming tone before a 1 ms transition to it. When
 * the audio thread is the one that stops the transition (the
 * "transition_stop" signal fires inside obs_transition_audio_render()), that
 * call must return no audio. It needs core
 * video, so it runs under xvfb-run and skips without a display.
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
/* Transition that records what obs_transition_audio_render() returns        */

static volatile long stop_ticks;
static volatile long removed_audio_ticks;

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

static float mix_a(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return 1.0f - t;
}

static float mix_b(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return t;
}

/* Set while this thread is inside obs_transition_audio_render(). */
static __thread bool in_audio_render;
static __thread bool audio_render_stopped;

/* "transition_stop" fires in the thread that stopped the transition. */
static void on_transition_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(cd);
	if (in_audio_render)
		audio_render_stopped = true;
}

static bool xfade_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			       size_t channels, size_t sample_rate)
{
	in_audio_render = true;
	audio_render_stopped = false;
	bool ret = obs_transition_audio_render(data, ts_out, audio, mixers, channels, sample_rate, mix_a, mix_b);
	in_audio_render = false;

	/* This call stopped the transition, which made the removed child A. */
	if (audio_render_stopped) {
		os_atomic_inc_long(&stop_ticks);
		if (ret)
			os_atomic_inc_long(&removed_audio_ticks);
	}
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

static void stop_tick_ignores_removed_child(void **state)
{
	UNUSED_PARAMETER(state);
	if (!core_ready)
		skip();

	obs_source_t *outgoing = obs_source_create("test_tone", "outgoing", NULL, NULL);
	obs_source_t *incoming = obs_source_create("test_tone", "incoming", NULL, NULL);
	obs_source_t *xfade = obs_source_create_private("test_xfade", "xfade", NULL);

	signal_handler_connect(obs_source_get_signal_handler(xfade), "transition_stop", on_transition_stop, NULL);
	obs_transition_set(xfade, outgoing);
	obs_set_output_source(0, xfade);
	os_sleep_ms(500);

	obs_source_remove(incoming);
	obs_transition_start(xfade, OBS_TRANSITION_MODE_AUTO, 1, incoming);
	os_sleep_ms(1000);

	obs_set_output_source(0, NULL);
	signal_handler_disconnect(obs_source_get_signal_handler(xfade), "transition_stop", on_transition_stop, NULL);
	obs_source_release(xfade);
	obs_source_remove(outgoing);
	obs_source_release(outgoing);
	obs_source_release(incoming);

	fprintf(stderr, "stop ticks: %ld, with audio from the removed child: %ld\n", os_atomic_load_long(&stop_ticks),
		os_atomic_load_long(&removed_audio_ticks));
	/* The video thread stops the transition instead when the audio has
	 * already finished; then this run cannot show the bug. */
	if (os_atomic_load_long(&stop_ticks) == 0)
		skip();
	assert_int_equal(os_atomic_load_long(&removed_audio_ticks), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(stop_tick_ignores_removed_child),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
