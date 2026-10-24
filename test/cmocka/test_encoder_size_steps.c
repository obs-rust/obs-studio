#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-size.h"

/*
 * Characterization of the encoder size and info choices of
 * libobs/obs-encoder.c, moved to libobs/obs-encoder-size.c before their Rust
 * port:
 * - obs_encoder_size_audio_defaults(): get_audio_info()'s default fill from
 *   the audio mix, before the plugin's get_audio_info;
 * - obs_encoder_size_cap_override(): get_video_info()'s
 *   OBS_ENCODER_CAP_SCALING override, after the plugin's get_video_info;
 * - obs_encoder_size_scaled_gate() and obs_encoder_size_scaled_match(): the
 *   refusals of obs_encoder_set_scaled_size() and its "matches the output,
 *   scaling disabled" rule;
 * - obs_encoder_size_scaling(): obs_encoder_scaling_enabled();
 * - obs_encoder_size_video_route(): where obs_encoder_get_width() and
 *   obs_encoder_get_height() take an axis from;
 * - obs_encoder_size_rate_route(): where obs_encoder_get_sample_rate() takes
 *   the rate from.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. The real-libobs
 * tests (test_encoder_video_reset, test_encoding_lag_rescale,
 * test_multitrack_encoder_override, test_encoder_fallback,
 * test_encoder_roi) cover the callers.
 *
 * Nothing here comes from outside OBS: the scaled sizes come from the
 * frontend and plugins, the caps, types and audio info from plugins, the
 * video and audio info from libobs.
 */

#define TYPE_AUDIO OBS_ENCODER_SIZE_TYPE_AUDIO
#define TYPE_VIDEO OBS_ENCODER_SIZE_TYPE_VIDEO
#define CAP_SCALING OBS_ENCODER_SIZE_CAP_SCALING

/* Types that are neither OBS_ENCODER_AUDIO nor OBS_ENCODER_VIDEO: a plugin's
 * info.type is not range-checked. */
static const int other_types[] = {2, 3, -1, INT_MAX, INT_MIN};
#define N_OTHER_TYPES (sizeof(other_types) / sizeof(other_types[0]))

static const uint32_t edge_sizes[] = {0,         1, 2, 720, 1080, 1280, 1920, 0x7fffffffu, 0x80000000u, UINT32_MAX - 1,
				      UINT32_MAX};
#define N_EDGE_SIZES (sizeof(edge_sizes) / sizeof(edge_sizes[0]))

/* ------------------------------------------------------------------------ */
/* obs_encoder_size_audio_defaults */

static struct obs_encoder_audio_fill fill(int format, uint32_t rate, int speakers)
{
	struct obs_encoder_audio_fill f = {format, rate, speakers};
	return f;
}

static void expect_fill(struct obs_encoder_audio_fill in, struct obs_encoder_audio_fill mix,
			struct obs_encoder_audio_fill want)
{
	struct obs_encoder_audio_fill got = obs_encoder_size_audio_defaults(&in, &mix);
	if (got.format != want.format || got.samples_per_sec != want.samples_per_sec || got.speakers != want.speakers) {
		fprintf(stderr, "defaults({%d, %u, %d}, mix {%d, %u, %d}): {%d, %u, %d}, expected {%d, %u, %d}\n",
			in.format, in.samples_per_sec, in.speakers, mix.format, mix.samples_per_sec, mix.speakers,
			got.format, got.samples_per_sec, got.speakers, want.format, want.samples_per_sec,
			want.speakers);
		fail();
	}
}

/* Both callers pass a zeroed info: every field comes from the mix. */
static void audio_defaults_all_unknown(void **state)
{
	(void)state;
	expect_fill(fill(0, 0, 0), fill(8, 48000, 2), fill(8, 48000, 2));
	expect_fill(fill(0, 0, 0), fill(1, 44100, 8), fill(1, 44100, 8));
}

static void audio_defaults_format_unknown(void **state)
{
	(void)state;
	expect_fill(fill(0, 44100, 1), fill(8, 48000, 2), fill(8, 44100, 1));
}

static void audio_defaults_rate_unknown(void **state)
{
	(void)state;
	expect_fill(fill(4, 0, 1), fill(8, 48000, 2), fill(4, 48000, 1));
}

static void audio_defaults_speakers_unknown(void **state)
{
	(void)state;
	expect_fill(fill(4, 44100, 0), fill(8, 48000, 2), fill(4, 44100, 2));
}

static void audio_defaults_none_unknown(void **state)
{
	(void)state;
	expect_fill(fill(4, 44100, 1), fill(8, 48000, 2), fill(4, 44100, 1));
}

/* An unknown field of the mix is copied as it is: still unknown. */
static void audio_defaults_mix_unknown(void **state)
{
	(void)state;
	expect_fill(fill(0, 0, 0), fill(0, 0, 0), fill(0, 0, 0));
	expect_fill(fill(4, 0, 0), fill(0, 0, 0), fill(4, 0, 0));
}

/* Every combination of known and unknown fields, against a model. */
static void audio_defaults_every_combination(void **state)
{
	(void)state;
	const struct obs_encoder_audio_fill mix = fill(8, 96000, 7);
	for (int bits = 0; bits < 8; bits++) {
		struct obs_encoder_audio_fill in = fill(bits & 1 ? 3 : 0, bits & 2 ? 22050 : 0, bits & 4 ? 4 : 0);
		struct obs_encoder_audio_fill want = fill(bits & 1 ? 3 : 8, bits & 2 ? 22050 : 96000, bits & 4 ? 4 : 7);
		expect_fill(in, mix, want);
	}
}

/* Any value other than 0 counts as known, values outside the enums
 * included; the rate at the u32 edges. */
static void audio_defaults_edges(void **state)
{
	(void)state;
	expect_fill(fill(-1, UINT32_MAX, -1), fill(8, 48000, 2), fill(-1, UINT32_MAX, -1));
	expect_fill(fill(INT_MAX, 1, INT_MIN), fill(8, 48000, 2), fill(INT_MAX, 1, INT_MIN));
	expect_fill(fill(0, 0, 0), fill(INT_MIN, UINT32_MAX, INT_MAX), fill(INT_MIN, UINT32_MAX, INT_MAX));
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_size_cap_override */

static void expect_override(uint32_t caps, uint32_t w, uint32_t h, uint32_t vw, uint32_t vh, uint32_t want_w,
			    uint32_t want_h)
{
	struct obs_encoder_size got = obs_encoder_size_cap_override(caps, w, h, vw, vh);
	if (got.width != want_w || got.height != want_h) {
		fprintf(stderr, "cap_override(%#x, %u x %u, voi %u x %u): %u x %u, expected %u x %u\n", caps, w, h, vw,
			vh, got.width, got.height, want_w, want_h);
		fail();
	}
}

/* Without the cap the size the plugin left is kept. */
static void cap_override_without_cap(void **state)
{
	(void)state;
	expect_override(0, 1280, 720, 1920, 1080, 1280, 720);
	expect_override(~(uint32_t)CAP_SCALING, 1280, 720, 1920, 1080, 1280, 720);
	expect_override(0, 0, 0, 1920, 1080, 0, 0);
}

/* With the cap the video's size wins, whatever the plugin set. */
static void cap_override_with_cap(void **state)
{
	(void)state;
	expect_override(CAP_SCALING, 1280, 720, 1920, 1080, 1920, 1080);
	expect_override(UINT32_MAX, 1280, 720, 1920, 1080, 1920, 1080);
	expect_override(CAP_SCALING, 1920, 1080, 1920, 1080, 1920, 1080);
	expect_override(CAP_SCALING, 0, 0, 1920, 1080, 1920, 1080);
}

/* Each cap bit alone: only OBS_ENCODER_CAP_SCALING (bit 5) overrides. */
static void cap_override_single_bits(void **state)
{
	(void)state;
	assert_int_equal(CAP_SCALING, 1 << 5);
	for (unsigned bit = 0; bit < 32; bit++) {
		uint32_t caps = (uint32_t)1 << bit;
		if (bit == 5)
			expect_override(caps, 640, 360, 1920, 1080, 1920, 1080);
		else
			expect_override(caps, 640, 360, 1920, 1080, 640, 360);
	}
}

/* Characterized, not endorsed: with the cap, a video of 0 x 0 gives 0 x 0,
 * whatever the plugin set. Every pair of edge sizes, with and without. */
static void cap_override_u32_edges(void **state)
{
	(void)state;
	expect_override(CAP_SCALING, 1280, 720, 0, 0, 0, 0);
	for (size_t i = 0; i < N_EDGE_SIZES; i++) {
		for (size_t j = 0; j < N_EDGE_SIZES; j++) {
			uint32_t a = edge_sizes[i], b = edge_sizes[j];
			expect_override(0, a, b, b, a, a, b);
			expect_override(CAP_SCALING, a, b, b, a, b, a);
		}
	}
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_size_scaled_gate */

static void expect_gate(int type, bool active, bool initialized, int want)
{
	int got = (int)obs_encoder_size_scaled_gate(type, active, initialized);
	if (got != want) {
		fprintf(stderr, "scaled_gate(%d, %d, %d): %d, expected %d\n", type, active, initialized, got, want);
		fail();
	}
}

/* Refused first for anything but a video encoder, active or initialized or
 * not. */
static void scaled_gate_not_video(void **state)
{
	(void)state;
	for (int f = 0; f < 4; f++) {
		expect_gate(TYPE_AUDIO, f & 1, f & 2, OBS_ENCODER_SCALED_NOT_VIDEO);
		for (size_t i = 0; i < N_OTHER_TYPES; i++)
			expect_gate(other_types[i], f & 1, f & 2, OBS_ENCODER_SCALED_NOT_VIDEO);
	}
}

/* An active video encoder is refused, initialized or not. */
static void scaled_gate_active(void **state)
{
	(void)state;
	expect_gate(TYPE_VIDEO, true, false, OBS_ENCODER_SCALED_ACTIVE);
	expect_gate(TYPE_VIDEO, true, true, OBS_ENCODER_SCALED_ACTIVE);
}

static void scaled_gate_initialized(void **state)
{
	(void)state;
	expect_gate(TYPE_VIDEO, false, true, OBS_ENCODER_SCALED_INITIALIZED);
}

static void scaled_gate_check(void **state)
{
	(void)state;
	expect_gate(TYPE_VIDEO, false, false, OBS_ENCODER_SCALED_CHECK);
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_size_scaled_match */

static void expect_match(bool has_voi, uint32_t vw, uint32_t vh, uint32_t w, uint32_t h, int want)
{
	int got = (int)obs_encoder_size_scaled_match(has_voi, vw, vh, w, h);
	if (got != want) {
		fprintf(stderr, "scaled_match(%d, voi %u x %u, %u x %u): %d, expected %d\n", has_voi, vw, vh, w, h, got,
			want);
		fail();
	}
}

/* The video's own size disables scaling (scaled size 0 x 0). */
static void scaled_match_equal(void **state)
{
	(void)state;
	expect_match(true, 1920, 1080, 1920, 1080, OBS_ENCODER_SCALED_DISABLE);
	expect_match(true, 1280, 720, 1280, 720, OBS_ENCODER_SCALED_DISABLE);
}

/* One axis differing is enough to set it. Characterized, not endorsed: a
 * size equal to the video's on one axis is set as it is, and so is a size
 * with one axis 0 (then obs_encoder_get_width() or _height() reads the
 * video's on that axis, and obs_encoder_scaling_enabled() is true). */
static void scaled_match_unequal(void **state)
{
	(void)state;
	expect_match(true, 1920, 1080, 1280, 720, OBS_ENCODER_SCALED_SET);
	expect_match(true, 1920, 1080, 1920, 720, OBS_ENCODER_SCALED_SET);
	expect_match(true, 1920, 1080, 1280, 1080, OBS_ENCODER_SCALED_SET);
	expect_match(true, 1920, 1080, 1080, 1920, OBS_ENCODER_SCALED_SET);
	expect_match(true, 1920, 1080, 0, 1080, OBS_ENCODER_SCALED_SET);
	expect_match(true, 1920, 1080, 1920, 0, OBS_ENCODER_SCALED_SET);
}

/* Without a video there is nothing to match: the size is set, the voi's
 * size (not read by the C) ignored. */
static void scaled_match_no_voi(void **state)
{
	(void)state;
	expect_match(false, 1920, 1080, 1920, 1080, OBS_ENCODER_SCALED_SET);
	expect_match(false, 0, 0, 0, 0, OBS_ENCODER_SCALED_SET);
	expect_match(false, 0, 0, 1280, 720, OBS_ENCODER_SCALED_SET);
}

/* Every pair of edge sizes: equal on both axes disables, anything else
 * sets. Asking for 0 x 0 sets 0 x 0, which turns scaling off anyway; 0 x 0
 * on a 0 x 0 video goes through the warning. */
static void scaled_match_u32_edges(void **state)
{
	(void)state;
	expect_match(true, 0, 0, 0, 0, OBS_ENCODER_SCALED_DISABLE);
	expect_match(true, 1920, 1080, 0, 0, OBS_ENCODER_SCALED_SET);
	for (size_t i = 0; i < N_EDGE_SIZES; i++) {
		for (size_t j = 0; j < N_EDGE_SIZES; j++) {
			uint32_t a = edge_sizes[i], b = edge_sizes[j];
			expect_match(true, a, b, a, b, OBS_ENCODER_SCALED_DISABLE);
			expect_match(true, a, b, b, a, a == b ? OBS_ENCODER_SCALED_DISABLE : OBS_ENCODER_SCALED_SET);
			expect_match(true, a, a, a, b, a == b ? OBS_ENCODER_SCALED_DISABLE : OBS_ENCODER_SCALED_SET);
			expect_match(false, a, b, a, b, OBS_ENCODER_SCALED_SET);
		}
	}
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_size_scaling */

static void scaling_enabled(void **state)
{
	(void)state;
	assert_false(obs_encoder_size_scaling(0, 0));
	/* either axis is enough */
	assert_true(obs_encoder_size_scaling(1280, 0));
	assert_true(obs_encoder_size_scaling(0, 720));
	assert_true(obs_encoder_size_scaling(1280, 720));
	assert_true(obs_encoder_size_scaling(1, 0));
	assert_true(obs_encoder_size_scaling(0, 1));
	assert_true(obs_encoder_size_scaling(UINT32_MAX, 0));
	assert_true(obs_encoder_size_scaling(0, UINT32_MAX));
	assert_true(obs_encoder_size_scaling(UINT32_MAX, UINT32_MAX));
	assert_true(obs_encoder_size_scaling(0x80000000u, 0));
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_size_video_route, obs_encoder_size_rate_route */

static void expect_video_route(int type, bool has_media, uint32_t scaled, int want)
{
	int got = (int)obs_encoder_size_video_route(type, has_media, scaled);
	if (got != want) {
		fprintf(stderr, "video_route(%d, %d, %u): %d, expected %d\n", type, has_media, scaled, got, want);
		fail();
	}
}

static void expect_rate_route(int type, bool has_media, uint32_t rate, int want)
{
	int got = (int)obs_encoder_size_rate_route(type, has_media, rate);
	if (got != want) {
		fprintf(stderr, "rate_route(%d, %d, %u): %d, expected %d\n", type, has_media, rate, got, want);
		fail();
	}
}

/* Anything but a video encoder is refused with a warning, before the media
 * is looked at. */
static void video_route_not_video(void **state)
{
	(void)state;
	for (size_t s = 0; s < N_EDGE_SIZES; s++) {
		for (int m = 0; m < 2; m++) {
			expect_video_route(TYPE_AUDIO, m, edge_sizes[s], OBS_ENCODER_SIZE_WRONG_TYPE);
			for (size_t i = 0; i < N_OTHER_TYPES; i++)
				expect_video_route(other_types[i], m, edge_sizes[s], OBS_ENCODER_SIZE_WRONG_TYPE);
		}
	}
}

/* A video encoder without a video reads 0, scaled or not. */
static void video_route_no_media(void **state)
{
	(void)state;
	for (size_t s = 0; s < N_EDGE_SIZES; s++)
		expect_video_route(TYPE_VIDEO, false, edge_sizes[s], OBS_ENCODER_SIZE_NO_MEDIA);
}

/* With a video: the scaled size when it is set, else the video's. */
static void video_route_scaled_or_video(void **state)
{
	(void)state;
	expect_video_route(TYPE_VIDEO, true, 0, OBS_ENCODER_SIZE_MEDIA);
	for (size_t s = 0; s < N_EDGE_SIZES; s++) {
		if (edge_sizes[s])
			expect_video_route(TYPE_VIDEO, true, edge_sizes[s], OBS_ENCODER_SIZE_STORED);
	}
}

static void rate_route_not_audio(void **state)
{
	(void)state;
	for (size_t s = 0; s < N_EDGE_SIZES; s++) {
		for (int m = 0; m < 2; m++) {
			expect_rate_route(TYPE_VIDEO, m, edge_sizes[s], OBS_ENCODER_SIZE_WRONG_TYPE);
			for (size_t i = 0; i < N_OTHER_TYPES; i++)
				expect_rate_route(other_types[i], m, edge_sizes[s], OBS_ENCODER_SIZE_WRONG_TYPE);
		}
	}
}

static void rate_route_no_media(void **state)
{
	(void)state;
	for (size_t s = 0; s < N_EDGE_SIZES; s++)
		expect_rate_route(TYPE_AUDIO, false, edge_sizes[s], OBS_ENCODER_SIZE_NO_MEDIA);
}

/* With an audio: the rate the encoder stored at initialization when it is
 * set, else the mix's. */
static void rate_route_stored_or_mix(void **state)
{
	(void)state;
	expect_rate_route(TYPE_AUDIO, true, 0, OBS_ENCODER_SIZE_MEDIA);
	const uint32_t rates[] = {1, 8000, 44100, 48000, 96000, UINT32_MAX};
	for (size_t s = 0; s < sizeof(rates) / sizeof(rates[0]); s++)
		expect_rate_route(TYPE_AUDIO, true, rates[s], OBS_ENCODER_SIZE_STORED);
}

/* The width and height of a video encoder, end to end, as
 * obs_encoder_get_width() / _height() pick them: with and without a scaled
 * size, with and without a video, and one axis scaled. */
static uint32_t pick_axis(int type, bool has_media, uint32_t scaled, uint32_t video)
{
	switch (obs_encoder_size_video_route(type, has_media, scaled)) {
	case OBS_ENCODER_SIZE_STORED:
		return scaled;
	case OBS_ENCODER_SIZE_MEDIA:
		return video;
	case OBS_ENCODER_SIZE_WRONG_TYPE:
	case OBS_ENCODER_SIZE_NO_MEDIA:
	default:
		return 0;
	}
}

static void width_height_end_to_end(void **state)
{
	(void)state;
	/* not scaled: the video's */
	assert_int_equal(pick_axis(TYPE_VIDEO, true, 0, 1920), 1920);
	assert_int_equal(pick_axis(TYPE_VIDEO, true, 0, 1080), 1080);
	/* scaled */
	assert_int_equal(pick_axis(TYPE_VIDEO, true, 1280, 1920), 1280);
	assert_int_equal(pick_axis(TYPE_VIDEO, true, 720, 1080), 720);
	/* scaled on one axis only (characterized, not endorsed) */
	assert_int_equal(pick_axis(TYPE_VIDEO, true, 1280, 1920), 1280);
	assert_int_equal(pick_axis(TYPE_VIDEO, true, 0, 1080), 1080);
	/* no video: 0, scaled or not */
	assert_int_equal(pick_axis(TYPE_VIDEO, false, 1280, 1920), 0);
	assert_int_equal(pick_axis(TYPE_VIDEO, false, 0, 1920), 0);
	/* an audio encoder: 0 */
	assert_int_equal(pick_axis(TYPE_AUDIO, true, 1280, 1920), 0);
	/* the u32 edges pass through */
	assert_int_equal(pick_axis(TYPE_VIDEO, true, UINT32_MAX, 1920), UINT32_MAX);
	assert_int_equal(pick_axis(TYPE_VIDEO, true, 0, UINT32_MAX), UINT32_MAX);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(audio_defaults_all_unknown),
		cmocka_unit_test(audio_defaults_format_unknown),
		cmocka_unit_test(audio_defaults_rate_unknown),
		cmocka_unit_test(audio_defaults_speakers_unknown),
		cmocka_unit_test(audio_defaults_none_unknown),
		cmocka_unit_test(audio_defaults_mix_unknown),
		cmocka_unit_test(audio_defaults_every_combination),
		cmocka_unit_test(audio_defaults_edges),
		cmocka_unit_test(cap_override_without_cap),
		cmocka_unit_test(cap_override_with_cap),
		cmocka_unit_test(cap_override_single_bits),
		cmocka_unit_test(cap_override_u32_edges),
		cmocka_unit_test(scaled_gate_not_video),
		cmocka_unit_test(scaled_gate_active),
		cmocka_unit_test(scaled_gate_initialized),
		cmocka_unit_test(scaled_gate_check),
		cmocka_unit_test(scaled_match_equal),
		cmocka_unit_test(scaled_match_unequal),
		cmocka_unit_test(scaled_match_no_voi),
		cmocka_unit_test(scaled_match_u32_edges),
		cmocka_unit_test(scaling_enabled),
		cmocka_unit_test(video_route_not_video),
		cmocka_unit_test(video_route_no_media),
		cmocka_unit_test(video_route_scaled_or_video),
		cmocka_unit_test(rate_route_not_audio),
		cmocka_unit_test(rate_route_no_media),
		cmocka_unit_test(rate_route_stored_or_mix),
		cmocka_unit_test(width_height_end_to_end),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
