#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-gpu-rescale.h"

/*
 * Characterization of the GPU rescale mix decisions of libobs/obs-encoder.c,
 * moved to libobs/obs-encoder-gpu-rescale.c before their Rust port (#3137):
 * - obs_encoder_gpu_rescale_wanted(): the gates of maybe_set_up_gpu_rescale();
 * - obs_encoder_gpu_rescale_target(): the output the encoder's mix must have;
 * - obs_encoder_gpu_rescale_matches(): whether a mix serves that output;
 * - obs_encoder_gpu_rescale_release_step() and
 *   obs_encoder_gpu_rescale_release_refs(): maybe_clear_encoder_core_video_mix().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. The real-libobs
 * tests (test_encoding_lag_rescale, test_output_rescale) cover the callers.
 *
 * Nothing here comes from outside OBS directly: the scale type, the scaled
 * size and the preferred format, colorspace and range come from the frontend,
 * plugins and scripts through the encoder API, the video outputs from libobs.
 */

#define DISABLE OBS_ENCODER_GPU_RESCALE_SCALE_DISABLE
#define FORMAT_NONE OBS_ENCODER_GPU_RESCALE_FORMAT_NONE
#define CS_DEFAULT OBS_ENCODER_GPU_RESCALE_CS_DEFAULT
#define RANGE_DEFAULT OBS_ENCODER_GPU_RESCALE_RANGE_DEFAULT

/* OBS_SCALE_BICUBIC; VIDEO_FORMAT_NV12, VIDEO_FORMAT_I420; VIDEO_CS_709,
 * VIDEO_CS_601; VIDEO_RANGE_PARTIAL, VIDEO_RANGE_FULL. */
#define BICUBIC 2
#define NV12 2
#define I420 1
#define CS_709 2
#define CS_601 1
#define PARTIAL 1
#define FULL 2

/* Enum values the C does not range-check: any value other than the "none"
 * or "default" one counts as set. */
static const int odd_enums[] = {-1, 99, INT_MAX, INT_MIN};
#define N_ODD_ENUMS (sizeof(odd_enums) / sizeof(odd_enums[0]))

static const uint32_t edge_sizes[] = {1, 2, 720, 1080, 0x7fffffffu, 0x80000000u, UINT32_MAX};
#define N_EDGE_SIZES (sizeof(edge_sizes) / sizeof(edge_sizes[0]))

static struct obs_encoder_gpu_rescale_request request(int scale_type, uint32_t width, uint32_t height, int format,
						      int colorspace, int range)
{
	struct obs_encoder_gpu_rescale_request r = {scale_type, width, height, format, colorspace, range};
	return r;
}

static struct obs_encoder_gpu_rescale_output output(uint32_t width, uint32_t height, int format, int colorspace,
						    int range)
{
	struct obs_encoder_gpu_rescale_output o = {width, height, format, colorspace, range};
	return o;
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_gpu_rescale_wanted */

static void expect_wanted(bool has_media, struct obs_encoder_gpu_rescale_request r, bool want)
{
	bool got = obs_encoder_gpu_rescale_wanted(has_media, &r);
	if (got != want) {
		fprintf(stderr, "wanted(media %d, {%d, %u, %u, %d, %d, %d}): %d, expected %d\n", has_media,
			r.scale_type, r.scaled_width, r.scaled_height, r.format, r.colorspace, r.range, got, want);
		fail();
	}
}

/* No media: no rescale, whatever the encoder asks for. */
static void wanted_no_media(void **state)
{
	(void)state;
	expect_wanted(false, request(BICUBIC, 1280, 720, NV12, CS_709, PARTIAL), false);
	expect_wanted(false, request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), false);
}

/* OBS_SCALE_DISABLE: no rescale, even with a scaled size and every
 * preference set. */
static void wanted_scale_disabled(void **state)
{
	(void)state;
	expect_wanted(true, request(DISABLE, 1280, 720, NV12, CS_709, PARTIAL), false);
	expect_wanted(true, request(DISABLE, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), false);
}

/* A scale type but nothing to do: no scaled size, no preferences. */
static void wanted_nothing_requested(void **state)
{
	(void)state;
	expect_wanted(true, request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), false);
}

/* Any one of the five requests is enough on its own. */
static void wanted_each_request_alone(void **state)
{
	(void)state;
	expect_wanted(true, request(BICUBIC, 1280, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), true);
	expect_wanted(true, request(BICUBIC, 0, 720, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), true);
	expect_wanted(true, request(BICUBIC, 0, 0, I420, CS_DEFAULT, RANGE_DEFAULT), true);
	expect_wanted(true, request(BICUBIC, 0, 0, FORMAT_NONE, CS_601, RANGE_DEFAULT), true);
	expect_wanted(true, request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, FULL), true);
	for (size_t i = 0; i < N_EDGE_SIZES; i++) {
		expect_wanted(true, request(BICUBIC, edge_sizes[i], 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), true);
		expect_wanted(true, request(BICUBIC, 0, edge_sizes[i], FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), true);
	}
}

/* Characterized, not endorsed: out-of-range enum values count as set, and
 * any scale type other than OBS_SCALE_DISABLE (even an unknown one) passes. */
static void wanted_odd_enums(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_ODD_ENUMS; i++) {
		const int v = odd_enums[i];
		expect_wanted(true, request(v, 1280, 720, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), true);
		expect_wanted(true, request(v, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), false);
		expect_wanted(true, request(BICUBIC, 0, 0, v, CS_DEFAULT, RANGE_DEFAULT), true);
		expect_wanted(true, request(BICUBIC, 0, 0, FORMAT_NONE, v, RANGE_DEFAULT), true);
		expect_wanted(true, request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, v), true);
		expect_wanted(false, request(v, 1280, 720, v, v, v), false);
	}
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_gpu_rescale_target */

static void expect_target(struct obs_encoder_gpu_rescale_request r, struct obs_encoder_gpu_rescale_output video,
			  struct obs_encoder_gpu_rescale_output want)
{
	struct obs_encoder_gpu_rescale_output got = obs_encoder_gpu_rescale_target(&r, &video);
	if (got.width != want.width || got.height != want.height || got.format != want.format ||
	    got.colorspace != want.colorspace || got.range != want.range) {
		fprintf(stderr,
			"target({%u, %u, %d, %d, %d}, video {%u, %u, %d, %d, %d}): {%u, %u, %d, %d, %d}, expected {%u, %u, %d, %d, %d}\n",
			r.scaled_width, r.scaled_height, r.format, r.colorspace, r.range, video.width, video.height,
			video.format, video.colorspace, video.range, got.width, got.height, got.format, got.colorspace,
			got.range, want.width, want.height, want.format, want.colorspace, want.range);
		fail();
	}
}

/* Nothing set: the video output's values. */
static void target_from_video(void **state)
{
	(void)state;
	const struct obs_encoder_gpu_rescale_output video = output(1920, 1080, NV12, CS_709, PARTIAL);
	expect_target(request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), video, video);
}

/* Everything set: the request's values. */
static void target_from_request(void **state)
{
	(void)state;
	expect_target(request(BICUBIC, 1280, 720, I420, CS_601, FULL), output(1920, 1080, NV12, CS_709, PARTIAL),
		      output(1280, 720, I420, CS_601, FULL));
}

/* Each field on its own: the others come from the video. */
static void target_each_field(void **state)
{
	(void)state;
	const struct obs_encoder_gpu_rescale_output video = output(1920, 1080, NV12, CS_709, PARTIAL);
	expect_target(request(BICUBIC, 1280, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), video,
		      output(1280, 1080, NV12, CS_709, PARTIAL));
	expect_target(request(BICUBIC, 0, 720, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), video,
		      output(1920, 720, NV12, CS_709, PARTIAL));
	expect_target(request(BICUBIC, 0, 0, I420, CS_DEFAULT, RANGE_DEFAULT), video,
		      output(1920, 1080, I420, CS_709, PARTIAL));
	expect_target(request(BICUBIC, 0, 0, FORMAT_NONE, CS_601, RANGE_DEFAULT), video,
		      output(1920, 1080, NV12, CS_601, PARTIAL));
	expect_target(request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, FULL), video,
		      output(1920, 1080, NV12, CS_709, FULL));
}

/* The u32 edges and odd enum values go through unchanged; the scale type
 * plays no part in the target. A video output with the "none" values gives
 * them back. */
static void target_edges(void **state)
{
	(void)state;
	const struct obs_encoder_gpu_rescale_output video = output(1920, 1080, NV12, CS_709, PARTIAL);
	for (size_t i = 0; i < N_EDGE_SIZES; i++) {
		const uint32_t s = edge_sizes[i];
		expect_target(request(BICUBIC, s, s, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), video,
			      output(s, s, NV12, CS_709, PARTIAL));
		expect_target(request(DISABLE, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), output(s, s, NV12, 0, 0),
			      output(s, s, NV12, 0, 0));
	}
	for (size_t i = 0; i < N_ODD_ENUMS; i++) {
		const int v = odd_enums[i];
		expect_target(request(v, 0, 0, v, v, v), video, output(1920, 1080, v, v, v));
	}
	expect_target(request(BICUBIC, 0, 0, FORMAT_NONE, CS_DEFAULT, RANGE_DEFAULT), output(0, 0, 0, 0, 0),
		      output(0, 0, 0, 0, 0));
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_gpu_rescale_matches */

static void expect_match(bool same_view, int mix_scale_type, struct obs_encoder_gpu_rescale_output mix, int scale_type,
			 struct obs_encoder_gpu_rescale_output target, bool want)
{
	bool got = obs_encoder_gpu_rescale_matches(same_view, mix_scale_type, &mix, scale_type, &target);
	if (got != want) {
		fprintf(stderr,
			"matches(view %d, scale %d, mix {%u, %u, %d, %d, %d}, scale %d, target {%u, %u, %d, %d, %d}): %d, expected %d\n",
			same_view, mix_scale_type, mix.width, mix.height, mix.format, mix.colorspace, mix.range,
			scale_type, target.width, target.height, target.format, target.colorspace, target.range, got,
			want);
		fail();
	}
}

/* Everything equal: the mix serves the target. */
static void matches_all_equal(void **state)
{
	(void)state;
	const struct obs_encoder_gpu_rescale_output t = output(1280, 720, NV12, CS_709, PARTIAL);
	expect_match(true, BICUBIC, t, BICUBIC, t, true);
	for (size_t i = 0; i < N_EDGE_SIZES; i++) {
		const struct obs_encoder_gpu_rescale_output e =
			output(edge_sizes[i], edge_sizes[i], -1, INT_MAX, INT_MIN);
		expect_match(true, -1, e, -1, e, true);
	}
}

/* Each condition on its own refuses the mix. */
static void matches_each_difference(void **state)
{
	(void)state;
	const struct obs_encoder_gpu_rescale_output t = output(1280, 720, NV12, CS_709, PARTIAL);
	expect_match(false, BICUBIC, t, BICUBIC, t, false);
	expect_match(true, BICUBIC + 1, t, BICUBIC, t, false);
	expect_match(true, DISABLE, t, BICUBIC, t, false);
	expect_match(true, BICUBIC, output(1281, 720, NV12, CS_709, PARTIAL), BICUBIC, t, false);
	expect_match(true, BICUBIC, output(1280, 721, NV12, CS_709, PARTIAL), BICUBIC, t, false);
	expect_match(true, BICUBIC, output(720, 1280, NV12, CS_709, PARTIAL), BICUBIC, t, false);
	expect_match(true, BICUBIC, output(1280, 720, I420, CS_709, PARTIAL), BICUBIC, t, false);
	expect_match(true, BICUBIC, output(1280, 720, NV12, CS_601, PARTIAL), BICUBIC, t, false);
	expect_match(true, BICUBIC, output(1280, 720, NV12, CS_709, FULL), BICUBIC, t, false);
	expect_match(true, BICUBIC, output(0, 0, 0, 0, 0), BICUBIC, t, false);
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_gpu_rescale_release_step, obs_encoder_gpu_rescale_release_refs */

static void release_step_each_input(void **state)
{
	(void)state;
	assert_int_equal(obs_encoder_gpu_rescale_release_step(false, false), OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP);
	assert_int_equal(obs_encoder_gpu_rescale_release_step(false, true), OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP);
	assert_int_equal(obs_encoder_gpu_rescale_release_step(true, false), OBS_ENCODER_GPU_RESCALE_RELEASE_STOP);
	assert_int_equal(obs_encoder_gpu_rescale_release_step(true, true), OBS_ENCODER_GPU_RESCALE_RELEASE_DROP);
}

static void expect_refs(long refs, long want, bool want_last)
{
	bool last = !want_last;
	long got = obs_encoder_gpu_rescale_release_refs(refs, &last);
	if (got != want || last != want_last) {
		fprintf(stderr, "release_refs(%ld): %ld, last %d, expected %ld, last %d\n", refs, got, last, want,
			want_last);
		fail();
	}
}

/* One reference goes; the last one frees the mix. Characterized, not
 * endorsed: a count at 0 goes to -1 and is not "last" (the C does not
 * guard it). */
static void release_refs_counts(void **state)
{
	(void)state;
	expect_refs(1, 0, true);
	expect_refs(2, 1, false);
	expect_refs(3, 2, false);
	expect_refs(0, -1, false);
	expect_refs(-1, -2, false);
	expect_refs(LONG_MAX, LONG_MAX - 1, false);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(wanted_no_media),
		cmocka_unit_test(wanted_scale_disabled),
		cmocka_unit_test(wanted_nothing_requested),
		cmocka_unit_test(wanted_each_request_alone),
		cmocka_unit_test(wanted_odd_enums),
		cmocka_unit_test(target_from_video),
		cmocka_unit_test(target_from_request),
		cmocka_unit_test(target_each_field),
		cmocka_unit_test(target_edges),
		cmocka_unit_test(matches_all_equal),
		cmocka_unit_test(matches_each_difference),
		cmocka_unit_test(release_step_each_input),
		cmocka_unit_test(release_refs_counts),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
