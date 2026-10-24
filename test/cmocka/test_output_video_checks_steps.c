#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-video-checks.h"

/*
 * Characterization of the output video size, conversion and capture checks
 * of libobs/obs-output.c, moved to libobs/obs-output-video-checks.c before
 * their Rust port:
 * - obs_output_first_mixer(): get_first_mixer(), the mixer a raw output's
 *   audio connects to;
 * - obs_output_size_route(): where obs_output_get_width2() and
 *   obs_output_get_height2() take one axis from;
 * - obs_output_capture_ready(): can_begin_data_capture(), the flag and
 *   presence checks (video_valid(), audio_valid(), the service);
 * - obs_output_begin_gate(): the delay-active and active gates of
 *   obs_output_can_begin_data_capture();
 * - obs_output_conversion_plan(): what obs_output_get_video_conversion()
 *   returns (the set conversion, filled; the scaled size; or none);
 * - obs_output_congestion_clamp(): the clamp of obs_output_get_congestion().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. The real-libobs
 * tests (test_raw_output_video_reset, test_scaled_mix_alpha,
 * test_texture_encoder_raw_format, test_output_start_twice,
 * test_output_failed_start_encoders, test_output_null_id,
 * test_encoding_lag_rescale) cover the callers.
 *
 * Nothing here comes from outside OBS: the sizes, flags and mixers come
 * from the frontend and plugins, the congestion from the output plugin.
 */

#define FLAG_VIDEO OBS_OUTPUT_VIDEO_CHECKS_VIDEO
#define FLAG_AUDIO OBS_OUTPUT_VIDEO_CHECKS_AUDIO
#define FLAG_ENCODED OBS_OUTPUT_VIDEO_CHECKS_ENCODED
#define FLAG_SERVICE OBS_OUTPUT_VIDEO_CHECKS_SERVICE

#define SIZE_BITS (sizeof(size_t) * 8)

/* ------------------------------------------------------------------------ */
/* obs_output_first_mixer */

static void first_mixer_zero(void **state)
{
	(void)state;
	/* no mixer: 0, the same as mixer 0 */
	assert_int_equal(obs_output_first_mixer(0), 0);
}

static void first_mixer_single_bits(void **state)
{
	(void)state;
	assert_int_equal(OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_MIXES, 6);
	for (size_t i = 0; i < SIZE_BITS; i++) {
		size_t want = i < OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_MIXES ? i : 0;
		size_t got = obs_output_first_mixer((size_t)1 << i);
		if (got != want)
			fprintf(stderr, "bit %zu: %zu, expected %zu\n", i, got, want);
		assert_int_equal(got, want);
	}
}

static void first_mixer_several_bits(void **state)
{
	(void)state;
	/* the lowest set bit wins */
	assert_int_equal(obs_output_first_mixer(0x3), 0);
	assert_int_equal(obs_output_first_mixer(0x6), 1);
	assert_int_equal(obs_output_first_mixer(0x28), 3);
	assert_int_equal(obs_output_first_mixer(0x30), 4);
	assert_int_equal(obs_output_first_mixer(0x3f), 0);
	assert_int_equal(obs_output_first_mixer(0x3e), 1);
	assert_int_equal(obs_output_first_mixer(0x20), 5);
	/* a bit past the mixers does not hide one below */
	assert_int_equal(obs_output_first_mixer(0x60), 5);
	assert_int_equal(obs_output_first_mixer(0x44), 2);
	assert_int_equal(obs_output_first_mixer(((size_t)1 << (SIZE_BITS - 1)) | 0x10), 4);
	assert_int_equal(obs_output_first_mixer(~(size_t)0), 0);
	assert_int_equal(obs_output_first_mixer(~(size_t)0x1f), 5);
	assert_int_equal(obs_output_first_mixer(~(size_t)0x7), 3);
}

/* Characterized, not endorsed: a mask with bits only at or above
 * MAX_AUDIO_MIXES (obs_output_set_mixers() with such a mask;
 * obs_output_set_mixer() ignores an index of 6 or more since #2127) reports
 * mixer 0, which the output does not use. */
static void first_mixer_high_bits_only(void **state)
{
	(void)state;
	assert_int_equal(obs_output_first_mixer(0x40), 0);
	assert_int_equal(obs_output_first_mixer(0x80), 0);
	assert_int_equal(obs_output_first_mixer(0xffc0), 0);
	assert_int_equal(obs_output_first_mixer(~(size_t)0x3f), 0);
	assert_int_equal(obs_output_first_mixer((size_t)1 << 31), 0);
	assert_int_equal(obs_output_first_mixer((size_t)1 << (SIZE_BITS - 1)), 0);
}

/* ------------------------------------------------------------------------ */
/* obs_output_size_route */

static void size_route_encoded(void **state)
{
	(void)state;
	/* an encoded output asks its encoder, whatever the scaled size */
	const uint32_t scaled[] = {0, 1, 1280, UINT32_MAX};
	for (size_t i = 0; i < sizeof(scaled) / sizeof(scaled[0]); i++) {
		assert_int_equal(obs_output_size_route(true, true, scaled[i]), OBS_OUTPUT_SIZE_ENCODER);
		assert_int_equal(obs_output_size_route(true, false, scaled[i]), OBS_OUTPUT_SIZE_ZERO);
	}
}

static void size_route_raw(void **state)
{
	(void)state;
	/* a raw output takes the scaled size when it is set, else the video's;
	 * an encoder at the index does not matter */
	const uint32_t scaled[] = {1, 2, 720, 1280, 0x80000000u, UINT32_MAX};
	for (size_t i = 0; i < sizeof(scaled) / sizeof(scaled[0]); i++) {
		assert_int_equal(obs_output_size_route(false, false, scaled[i]), OBS_OUTPUT_SIZE_SCALED);
		assert_int_equal(obs_output_size_route(false, true, scaled[i]), OBS_OUTPUT_SIZE_SCALED);
	}
	assert_int_equal(obs_output_size_route(false, false, 0), OBS_OUTPUT_SIZE_VIDEO);
	assert_int_equal(obs_output_size_route(false, true, 0), OBS_OUTPUT_SIZE_VIDEO);
}

/* ------------------------------------------------------------------------ */
/* obs_output_capture_ready */

static struct obs_output_capture_presence presence(uint32_t flags, uint32_t video_encoders, uint32_t audio_encoders,
						   bool video, bool audio, bool service)
{
	struct obs_output_capture_presence p = {flags, video_encoders, audio_encoders, video, audio, service};
	return p;
}

static bool ready(uint32_t flags, uint32_t video_encoders, uint32_t audio_encoders, bool video, bool audio,
		  bool service)
{
	struct obs_output_capture_presence p = presence(flags, video_encoders, audio_encoders, video, audio, service);
	return obs_output_capture_ready(&p);
}

static void capture_no_flags(void **state)
{
	(void)state;
	/* an output that wants nothing can always begin */
	assert_true(ready(0, 0, 0, false, false, false));
	assert_true(ready(0, 0x3ff, 0x3f, true, true, true));
	/* encoded alone wants nothing either */
	assert_true(ready(FLAG_ENCODED, 0, 0, false, false, false));
}

static void capture_raw_video(void **state)
{
	(void)state;
	/* a raw video output needs output->video, nothing else */
	assert_true(ready(FLAG_VIDEO, 0, 0, true, false, false));
	assert_false(ready(FLAG_VIDEO, 0, 0, false, false, false));
	/* encoders do not stand in for the video */
	assert_false(ready(FLAG_VIDEO, 0x3ff, 0x3f, false, true, true));
	assert_true(ready(FLAG_VIDEO, 0, 0, true, false, true));
}

static void capture_raw_audio(void **state)
{
	(void)state;
	assert_true(ready(FLAG_AUDIO, 0, 0, false, true, false));
	assert_false(ready(FLAG_AUDIO, 0, 0, false, false, false));
	assert_false(ready(FLAG_AUDIO, 0x3ff, 0x3f, true, false, true));
}

static void capture_raw_av(void **state)
{
	(void)state;
	assert_true(ready(FLAG_VIDEO | FLAG_AUDIO, 0, 0, true, true, false));
	assert_false(ready(FLAG_VIDEO | FLAG_AUDIO, 0, 0, true, false, false));
	assert_false(ready(FLAG_VIDEO | FLAG_AUDIO, 0, 0, false, true, false));
	assert_false(ready(FLAG_VIDEO | FLAG_AUDIO, 0, 0, false, false, false));
}

static void capture_encoded_video(void **state)
{
	(void)state;
	const uint32_t video = FLAG_VIDEO | FLAG_ENCODED;
	/* any one of the MAX_OUTPUT_VIDEO_ENCODERS encoders */
	assert_int_equal(OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS, 10);
	for (uint32_t i = 0; i < 32; i++) {
		bool want = i < OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS;
		bool got = ready(video, (uint32_t)1 << i, 0, false, false, false);
		if (got != want)
			fprintf(stderr, "video encoder bit %u: %d, expected %d\n", i, got, want);
		assert_int_equal(got, want);
	}
	assert_false(ready(video, 0, 0, false, false, false));
	assert_true(ready(video, 0x3ff, 0, false, false, false));
	assert_true(ready(video, 0x200, 0, false, false, false));
	/* the video does not stand in for an encoder */
	assert_false(ready(video, 0, 0x3f, true, true, true));
}

static void capture_encoded_audio(void **state)
{
	(void)state;
	const uint32_t audio = FLAG_AUDIO | FLAG_ENCODED;
	assert_int_equal(OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS, 6);
	for (uint32_t i = 0; i < 32; i++) {
		bool want = i < OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS;
		bool got = ready(audio, 0, (uint32_t)1 << i, false, false, false);
		if (got != want)
			fprintf(stderr, "audio encoder bit %u: %d, expected %d\n", i, got, want);
		assert_int_equal(got, want);
	}
	assert_false(ready(audio, 0, 0, false, false, false));
	assert_true(ready(audio, 0, 0x3f, false, false, false));
	assert_true(ready(audio, 0, 0x20, false, false, false));
	assert_false(ready(audio, 0x3ff, 0, true, true, true));
}

/* Characterized: an encoder bit at or above the limit is not an encoder.
 * obs-output.c builds the masks from video_encoders[] and audio_encoders[],
 * so it never sets one. */
static void capture_encoder_bits_past_the_limit(void **state)
{
	(void)state;
	assert_false(ready(FLAG_VIDEO | FLAG_ENCODED, ~(uint32_t)0x3ff, 0, true, true, true));
	assert_false(ready(FLAG_AUDIO | FLAG_ENCODED, 0, ~(uint32_t)0x3f, true, true, true));
	assert_true(ready(FLAG_VIDEO | FLAG_ENCODED, ~(uint32_t)0x1ff, 0, false, false, false));
	assert_true(ready(FLAG_AUDIO | FLAG_ENCODED, 0, ~(uint32_t)0x1f, false, false, false));
}

static void capture_service(void **state)
{
	(void)state;
	assert_true(ready(FLAG_SERVICE, 0, 0, false, false, true));
	assert_false(ready(FLAG_SERVICE, 0, 0, false, false, false));
	assert_false(ready(FLAG_SERVICE | FLAG_ENCODED, 0x3ff, 0x3f, true, true, false));
	/* a stream: encoded video and audio and a service */
	const uint32_t stream = FLAG_VIDEO | FLAG_AUDIO | FLAG_ENCODED | FLAG_SERVICE;
	assert_true(ready(stream, 0x1, 0x1, false, false, true));
	assert_false(ready(stream, 0x1, 0x1, true, true, false));
	assert_false(ready(stream, 0x0, 0x1, true, true, true));
	assert_false(ready(stream, 0x1, 0x0, true, true, true));
}

/* Only the four flags matter: OBS_OUTPUT_MULTI_TRACK, _CAN_PAUSE,
 * _MULTI_TRACK_VIDEO and unknown bits change nothing. */
static void capture_other_flags_ignored(void **state)
{
	(void)state;
	const uint32_t others = ~(uint32_t)(FLAG_VIDEO | FLAG_AUDIO | FLAG_ENCODED | FLAG_SERVICE);
	assert_true(ready(others, 0, 0, false, false, false));
	assert_true(ready(others | FLAG_VIDEO, 0, 0, true, false, false));
	assert_false(ready(others | FLAG_VIDEO, 0, 0, false, false, false));
	assert_true(ready(others | FLAG_VIDEO | FLAG_ENCODED, 0x1, 0, false, false, false));
}

/* The model: each wanted thing must be there, video and audio from the
 * encoders when encoded, from the video and audio otherwise. */
static bool model_ready(const struct obs_output_capture_presence *p)
{
	bool encoded = (p->flags & FLAG_ENCODED) != 0;
	bool has_video = encoded ? (p->video_encoders & 0x3ff) != 0 : p->video;
	bool has_audio = encoded ? (p->audio_encoders & 0x3f) != 0 : p->audio;
	if ((p->flags & FLAG_VIDEO) && !has_video)
		return false;
	if ((p->flags & FLAG_AUDIO) && !has_audio)
		return false;
	if ((p->flags & FLAG_SERVICE) && !p->service)
		return false;
	return true;
}

/* Every combination of the four flags against every presence bit: each
 * video encoder bit and each audio encoder bit alone (and none, and past
 * the limit), the video, the audio and the service. */
static void capture_every_flag_against_every_presence(void **state)
{
	(void)state;
	uint32_t video_masks[OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS + 3];
	uint32_t audio_masks[OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS + 3];
	size_t nv = 0, na = 0;
	video_masks[nv++] = 0;
	for (uint32_t i = 0; i < OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS; i++)
		video_masks[nv++] = (uint32_t)1 << i;
	video_masks[nv++] = (uint32_t)1 << OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS;
	video_masks[nv++] = (uint32_t)1 << 31;
	audio_masks[na++] = 0;
	for (uint32_t i = 0; i < OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS; i++)
		audio_masks[na++] = (uint32_t)1 << i;
	audio_masks[na++] = (uint32_t)1 << OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS;
	audio_masks[na++] = (uint32_t)1 << 31;

	size_t checked = 0, true_count = 0;
	for (uint32_t flags = 0; flags < 16; flags++) {
		for (size_t v = 0; v < nv; v++) {
			for (size_t a = 0; a < na; a++) {
				for (uint32_t bits = 0; bits < 8; bits++) {
					struct obs_output_capture_presence p =
						presence(flags, video_masks[v], audio_masks[a], (bits & 1) != 0,
							 (bits & 2) != 0, (bits & 4) != 0);
					bool got = obs_output_capture_ready(&p);
					bool want = model_ready(&p);
					if (got != want)
						fprintf(stderr,
							"flags %#x video_encoders %#x audio_encoders %#x video %d "
							"audio %d service %d: %d, model %d\n",
							flags, p.video_encoders, p.audio_encoders, p.video, p.audio,
							p.service, got, want);
					assert_int_equal(got, want);
					checked++;
					true_count += got;
				}
			}
		}
	}
	assert_int_equal(checked, 16 * 13 * 9 * 8);
	/* both answers are reached many times */
	assert_true(true_count > 1000);
	assert_true(checked - true_count > 1000);
}

/* ------------------------------------------------------------------------ */
/* obs_output_begin_gate */

static void begin_gate(void **state)
{
	(void)state;
	/* a delayed output (re)starts its own capture: true, even while active */
	assert_int_equal(obs_output_begin_gate(true, false), OBS_OUTPUT_BEGIN_READY);
	assert_int_equal(obs_output_begin_gate(true, true), OBS_OUTPUT_BEGIN_READY);
	/* an active output cannot begin again */
	assert_int_equal(obs_output_begin_gate(false, true), OBS_OUTPUT_BEGIN_REFUSE);
	/* otherwise: join the ending capture, then the presence checks */
	assert_int_equal(obs_output_begin_gate(false, false), OBS_OUTPUT_BEGIN_CHECK);
}

/* ------------------------------------------------------------------------ */
/* obs_output_conversion_plan */

static struct obs_output_conversion_choice plan(bool set, uint32_t set_width, uint32_t set_height,
						uint32_t scaled_width, uint32_t scaled_height, uint32_t video_width,
						uint32_t video_height)
{
	struct obs_output_conversion_sizes s = {set,           set_width,   set_height,  scaled_width,
						scaled_height, video_width, video_height};
	return obs_output_conversion_plan(&s);
}

static void expect_plan(struct obs_output_conversion_choice got, int kind, uint32_t width, uint32_t height)
{
	if (got.kind != kind || got.width != width || got.height != height)
		fprintf(stderr, "plan %d %ux%u, expected %d %ux%u\n", got.kind, got.width, got.height, kind, width,
			height);
	assert_int_equal(got.kind, kind);
	assert_int_equal(got.width, width);
	assert_int_equal(got.height, height);
}

static void conversion_not_set(void **state)
{
	(void)state;
	/* no scaled size: none */
	expect_plan(plan(false, 0, 0, 0, 0, 1920, 1080), OBS_OUTPUT_CONVERSION_NONE, 0, 0);
	/* the scaled size equal to the video's: none */
	expect_plan(plan(false, 0, 0, 1920, 1080, 1920, 1080), OBS_OUTPUT_CONVERSION_NONE, 0, 0);
	/* different: the scaled size */
	expect_plan(plan(false, 0, 0, 1280, 720, 1920, 1080), OBS_OUTPUT_CONVERSION_SCALED, 1280, 720);
	expect_plan(plan(false, 0, 0, 3840, 2160, 1920, 1080), OBS_OUTPUT_CONVERSION_SCALED, 3840, 2160);
	/* one axis different is enough */
	expect_plan(plan(false, 0, 0, 1920, 1081, 1920, 1080), OBS_OUTPUT_CONVERSION_SCALED, 1920, 1081);
	expect_plan(plan(false, 0, 0, 1919, 1080, 1920, 1080), OBS_OUTPUT_CONVERSION_SCALED, 1919, 1080);
	/* a stale conversion size is ignored when it is not set */
	expect_plan(plan(false, 640, 360, 1280, 720, 1920, 1080), OBS_OUTPUT_CONVERSION_SCALED, 1280, 720);
	expect_plan(plan(false, 640, 360, 0, 0, 1920, 1080), OBS_OUTPUT_CONVERSION_NONE, 0, 0);
}

/* A scaled size counts only with both axes: one axis 0 is no scaling. */
static void conversion_scaled_one_axis_zero(void **state)
{
	(void)state;
	expect_plan(plan(false, 0, 0, 1280, 0, 1920, 1080), OBS_OUTPUT_CONVERSION_NONE, 0, 0);
	expect_plan(plan(false, 0, 0, 0, 720, 1920, 1080), OBS_OUTPUT_CONVERSION_NONE, 0, 0);
}

/* No video (output->video NULL after a view's mix is gone, #470, reads as
 * 0 x 0): any scaled size differs. */
static void conversion_no_video(void **state)
{
	(void)state;
	expect_plan(plan(false, 0, 0, 1280, 720, 0, 0), OBS_OUTPUT_CONVERSION_SCALED, 1280, 720);
	expect_plan(plan(false, 0, 0, 0, 0, 0, 0), OBS_OUTPUT_CONVERSION_NONE, 0, 0);
}

static void conversion_set(void **state)
{
	(void)state;
	/* the set conversion, as it is, whatever the scaled and video sizes */
	expect_plan(plan(true, 640, 360, 0, 0, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 640, 360);
	expect_plan(plan(true, 640, 360, 1280, 720, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 640, 360);
	expect_plan(plan(true, 640, 360, 1920, 1080, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 640, 360);
	expect_plan(plan(true, 1920, 1080, 1920, 1080, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 1920, 1080);
	expect_plan(plan(true, UINT32_MAX, 1, 0, 0, 0, 0), OBS_OUTPUT_CONVERSION_SET, UINT32_MAX, 1);
}

/* A set conversion with a 0 width or height takes the output's size on that
 * axis (obs_output_get_width() / _height() of a raw output): the scaled size
 * when it is set, else the video's. Each axis on its own. */
static void conversion_set_zero_filled(void **state)
{
	(void)state;
	expect_plan(plan(true, 0, 0, 0, 0, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 1920, 1080);
	expect_plan(plan(true, 0, 0, 1280, 720, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 1280, 720);
	expect_plan(plan(true, 0, 360, 1280, 720, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 1280, 360);
	expect_plan(plan(true, 640, 0, 1280, 720, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 640, 720);
	expect_plan(plan(true, 0, 360, 0, 0, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 1920, 360);
	expect_plan(plan(true, 640, 0, 0, 0, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 640, 1080);
	/* the fill takes each axis's own scaled size, even when the other is 0 */
	expect_plan(plan(true, 0, 0, 1280, 0, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 1280, 1080);
	expect_plan(plan(true, 0, 0, 0, 720, 1920, 1080), OBS_OUTPUT_CONVERSION_SET, 1920, 720);
}

/* Characterized, not endorsed: with no scaled size and no video, a 0 width
 * or height stays 0, and the set conversion is still returned. */
static void conversion_set_zero_without_video(void **state)
{
	(void)state;
	expect_plan(plan(true, 0, 0, 0, 0, 0, 0), OBS_OUTPUT_CONVERSION_SET, 0, 0);
	expect_plan(plan(true, 640, 0, 0, 0, 0, 0), OBS_OUTPUT_CONVERSION_SET, 640, 0);
}

/* ------------------------------------------------------------------------ */
/* obs_output_congestion_clamp */

static uint32_t float_bits(float f)
{
	uint32_t b;
	memcpy(&b, &f, sizeof(b));
	return b;
}

static float bits_float(uint32_t b)
{
	float f;
	memcpy(&f, &b, sizeof(f));
	return f;
}

/* Bit for bit: -0 is not +0, and a NaN keeps its payload. */
static void expect_clamp(float in, float want)
{
	float got = obs_output_congestion_clamp(in);
	if (float_bits(got) != float_bits(want))
		fprintf(stderr, "clamp(%g [%#010x]): %g [%#010x], expected %g [%#010x]\n", in, float_bits(in), got,
			float_bits(got), want, float_bits(want));
	assert_int_equal(float_bits(got), float_bits(want));
}

static void congestion_range(void **state)
{
	(void)state;
	expect_clamp(-1.0f, 0.0f);
	expect_clamp(0.0f, 0.0f);
	expect_clamp(0.5f, 0.5f);
	expect_clamp(1.0f, 1.0f);
	expect_clamp(2.0f, 1.0f);
	expect_clamp(0.25f, 0.25f);
	expect_clamp(bits_float(0x3f7fffff), bits_float(0x3f7fffff));
	expect_clamp(bits_float(0x3f800001), 1.0f);
	expect_clamp(bits_float(0x00000001), bits_float(0x00000001));
	/* the smallest negative float is below 0 */
	expect_clamp(bits_float(0x80000001), 0.0f);
	expect_clamp(-3.4028235e38f, 0.0f);
	expect_clamp(3.4028235e38f, 1.0f);
}

static void congestion_infinities(void **state)
{
	(void)state;
	expect_clamp(INFINITY, 1.0f);
	expect_clamp(-INFINITY, 0.0f);
}

/* -0 is not below 0, so it passes through as -0. */
static void congestion_negative_zero(void **state)
{
	(void)state;
	expect_clamp(-0.0f, -0.0f);
	assert_int_equal(float_bits(obs_output_congestion_clamp(-0.0f)), 0x80000000u);
}

/* Characterized, not endorsed: a NaN from the plugin is neither below 0 nor
 * above 1, so obs_output_get_congestion() returns it, payload and sign
 * kept. */
static void congestion_nan(void **state)
{
	(void)state;
	assert_true(isnan(obs_output_congestion_clamp(NAN)));
	const uint32_t nans[] = {0x7fc00000u, 0xffc00000u, 0x7fc00001u, 0x7fffffffu, 0xffffffffu};
	for (size_t i = 0; i < sizeof(nans) / sizeof(nans[0]); i++)
		assert_int_equal(float_bits(obs_output_congestion_clamp(bits_float(nans[i]))), nans[i]);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(first_mixer_zero),
		cmocka_unit_test(first_mixer_single_bits),
		cmocka_unit_test(first_mixer_several_bits),
		cmocka_unit_test(first_mixer_high_bits_only),
		cmocka_unit_test(size_route_encoded),
		cmocka_unit_test(size_route_raw),
		cmocka_unit_test(capture_no_flags),
		cmocka_unit_test(capture_raw_video),
		cmocka_unit_test(capture_raw_audio),
		cmocka_unit_test(capture_raw_av),
		cmocka_unit_test(capture_encoded_video),
		cmocka_unit_test(capture_encoded_audio),
		cmocka_unit_test(capture_encoder_bits_past_the_limit),
		cmocka_unit_test(capture_service),
		cmocka_unit_test(capture_other_flags_ignored),
		cmocka_unit_test(capture_every_flag_against_every_presence),
		cmocka_unit_test(begin_gate),
		cmocka_unit_test(conversion_not_set),
		cmocka_unit_test(conversion_scaled_one_axis_zero),
		cmocka_unit_test(conversion_no_video),
		cmocka_unit_test(conversion_set),
		cmocka_unit_test(conversion_set_zero_filled),
		cmocka_unit_test(conversion_set_zero_without_video),
		cmocka_unit_test(congestion_range),
		cmocka_unit_test(congestion_infinities),
		cmocka_unit_test(congestion_negative_zero),
		cmocka_unit_test(congestion_nan),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
