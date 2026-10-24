#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-video-output-step.h"

/*
 * Characterization of the per-frame video output step decisions of
 * libobs/obs-video.c, moved to libobs/obs-video-output-step.c before their
 * Rust port:
 * - obs_video_step_activation(): update_active_state(), which frame data to
 *   clear and the new *_was_active flags;
 * - obs_video_step_stage(): stage_output_texture(), what is staged;
 * - obs_video_step_queue(), _queue_copy_uv() and _queue_count():
 *   queue_frame(), duplicate, copy or swap, the separate UV copy and the
 *   count left;
 * - obs_video_step_render_targets() and obs_video_step_render():
 *   render_video()'s steps;
 * - obs_video_step_ring(), _download() and _output(): output_frame()'s
 *   texture ring and its download and output.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. obs-video.c keeps
 * every gs_* call, the deques, the semaphore and the host calls; the
 * real-libobs tests (test_video_output_formats, test_raw_output_video_reset,
 * test_encoding_lag_rescale, test_scaled_mix_alpha, test_video_output_plan,
 * test_texture_encoder_raw_format and the GPU encoder tests) cover the
 * callers.
 *
 * Nothing here comes from outside OBS: the flags and counts come from the
 * outputs and encoders libobs starts, the queue sizes from its own deques.
 */

#define FRAME 48 /* sizeof(struct obs_tex_frame) on LP64; any non-zero size */

/* ------------------------------------------------------------------------ */
/* helpers */

static void check_activation(const char *what, struct obs_video_step_activation got, bool base, bool raw, bool gpu,
			     bool raw_active, bool gpu_active, bool active)
{
	if (got.clear_base != base || got.clear_raw != raw || got.clear_gpu != gpu || got.raw_active != raw_active ||
	    got.gpu_active != gpu_active || got.active != active) {
		fprintf(stderr,
			"%s: clear base %d raw %d gpu %d, active raw %d gpu %d any %d; "
			"expected clear base %d raw %d gpu %d, active raw %d gpu %d any %d\n",
			what, got.clear_base, got.clear_raw, got.clear_gpu, got.raw_active, got.gpu_active, got.active,
			base, raw, gpu, raw_active, gpu_active, active);
		fail();
	}
}

static void check_stage(const char *what, struct obs_video_step_stage got, bool staged, bool from_output,
			size_t channels)
{
	if (got.staged != staged || got.from_output != from_output || got.channels != channels) {
		fprintf(stderr, "%s: staged %d from output %d channels %zu; expected %d %d %zu\n", what, got.staged,
			got.from_output, got.channels, staged, from_output, channels);
		fail();
	}
}

static void check_queue(size_t avail, size_t queue, int count, bool raw, enum obs_video_step_queue_action expected)
{
	enum obs_video_step_queue_action got = obs_video_step_queue(avail, queue, count, raw);
	if (got != expected) {
		fprintf(stderr, "queue avail %zu queue %zu count %d raw %d: %d, expected %d\n", avail, queue, count,
			raw, (int)got, (int)expected);
		fail();
	}
}

static void check_ring(int cur, int prev, int next)
{
	struct obs_video_step_ring got = obs_video_step_ring(cur);
	if (got.prev != prev || got.next != next) {
		fprintf(stderr, "ring %d: prev %d next %d, expected %d %d\n", cur, got.prev, got.next, prev, next);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* update_active_state() */

static void steps_constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_VIDEO_STEP_TEXTURES, 2);
	assert_int_equal(OBS_VIDEO_STEP_CHANNELS, 3);
	assert_int_equal(OBS_VIDEO_STEP_QUEUE_DUPLICATE, 0);
	assert_int_equal(OBS_VIDEO_STEP_QUEUE_COPY, 1);
	assert_int_equal(OBS_VIDEO_STEP_QUEUE_SWAP, 2);
}

/* Every combination of the three was flags and of the two atomics as off
 * (0), on (1) and the other values the counters could hold. */
static void steps_activation_every_combination(void **state)
{
	(void)state;
	const long values[] = {LONG_MIN, -2, -1, 0, 1, 2, 3, LONG_MAX};
	for (int was = 0; was < 8; was++) {
		for (size_t r = 0; r < sizeof(values) / sizeof(values[0]); r++) {
			for (size_t g = 0; g < sizeof(values) / sizeof(values[0]); g++) {
				const struct obs_video_step_activation_input input = {
					.raw_was_active = was & 1,
					.gpu_was_active = (was >> 1) & 1,
					.was_active = (was >> 2) & 1,
					.raw_active = values[r],
					.gpu_encoder_active = values[g],
				};
				const bool raw = values[r] > 0;
				const bool gpu = values[g] > 0;
				const bool any = raw || gpu;
				char what[128];
				snprintf(what, sizeof(what), "was %d%d%d raw %ld gpu %ld", input.raw_was_active,
					 input.gpu_was_active, input.was_active, values[r], values[g]);
				check_activation(what, obs_video_step_activation(&input), !input.was_active && any,
						 !input.raw_was_active && raw, !input.gpu_was_active && gpu, raw, gpu,
						 any);
			}
		}
	}
}

/* The edges one at a time: raw output starting clears the base and the raw
 * data; GPU encoding starting with raw output already on clears only the
 * GPU data; stopping clears nothing. */
static void steps_activation_edges(void **state)
{
	(void)state;
	struct obs_video_step_activation_input input = {0};

	input.raw_active = 1;
	check_activation("raw starts", obs_video_step_activation(&input), true, true, false, true, false, true);

	input = (struct obs_video_step_activation_input){.raw_was_active = true, .was_active = true, .raw_active = 1};
	check_activation("raw stays", obs_video_step_activation(&input), false, false, false, true, false, true);

	input.gpu_encoder_active = 1;
	check_activation("gpu starts with raw on", obs_video_step_activation(&input), false, false, true, true, true,
			 true);

	input = (struct obs_video_step_activation_input){.gpu_encoder_active = 2};
	check_activation("gpu starts alone", obs_video_step_activation(&input), true, false, true, false, true, true);

	input = (struct obs_video_step_activation_input){.raw_was_active = true,
							 .gpu_was_active = true,
							 .was_active = true};
	check_activation("both stop", obs_video_step_activation(&input), false, false, false, false, false, false);

	/* A negative counter is off. */
	input = (struct obs_video_step_activation_input){.raw_active = -1, .gpu_encoder_active = LONG_MIN};
	check_activation("negative counters", obs_video_step_activation(&input), false, false, false, false, false,
			 false);
}

/* Characterized, not endorsed: the was flags are taken as they are, even
 * when was_active disagrees with the other two. raw_was_active without
 * was_active still clears the base data when raw output is on. */
static void steps_activation_inconsistent_was_flags(void **state)
{
	(void)state;
	struct obs_video_step_activation_input input = {.raw_was_active = true, .raw_active = 1};
	check_activation("raw was, any was not", obs_video_step_activation(&input), true, false, false, true, false,
			 true);

	input = (struct obs_video_step_activation_input){.was_active = true, .gpu_encoder_active = 1};
	check_activation("any was, gpu was not", obs_video_step_activation(&input), false, false, true, false, true,
			 true);
}

/* ------------------------------------------------------------------------ */
/* stage_output_texture() */

/* Without GPU conversion: copy surface 0 from the output texture, whatever
 * the channel count and the converted flag. */
static void steps_stage_without_conversion(void **state)
{
	(void)state;
	for (size_t channels = 0; channels <= OBS_VIDEO_STEP_CHANNELS + 1; channels++) {
		for (int converted = 0; converted < 2; converted++) {
			char what[64];
			snprintf(what, sizeof(what), "no conversion channels %zu converted %d", channels, converted);
			check_stage(what, obs_video_step_stage(false, converted, channels), true, true, 1);
		}
	}
	check_stage("no conversion SIZE_MAX", obs_video_step_stage(false, true, SIZE_MAX), true, true, 1);
}

/* With GPU conversion, once converted: channel_count copy surfaces from the
 * convert textures, for each count from 0 to NUM_CHANNELS. */
static void steps_stage_with_conversion(void **state)
{
	(void)state;
	for (size_t channels = 0; channels <= OBS_VIDEO_STEP_CHANNELS; channels++) {
		char what[64];
		snprintf(what, sizeof(what), "converted channels %zu", channels);
		check_stage(what, obs_video_step_stage(true, true, channels), true, false, channels);
	}
}

/* With GPU conversion, not converted yet: nothing is staged and
 * textures_copied stays as it was. */
static void steps_stage_not_converted(void **state)
{
	(void)state;
	for (size_t channels = 0; channels <= OBS_VIDEO_STEP_CHANNELS; channels++) {
		char what[64];
		snprintf(what, sizeof(what), "unconverted channels %zu", channels);
		check_stage(what, obs_video_step_stage(true, false, channels), false, false, 0);
	}
}

/* Characterized, not endorsed: a channel count past NUM_CHANNELS is passed
 * through; obs-video.c would index past its arrays. The hosts set 1 (Windows)
 * or leave NUM_CHANNELS. */
static void steps_stage_count_past_channels(void **state)
{
	(void)state;
	check_stage("converted 4", obs_video_step_stage(true, true, 4), true, false, 4);
	check_stage("converted SIZE_MAX", obs_video_step_stage(true, true, SIZE_MAX), true, false, SIZE_MAX);
}

/* ------------------------------------------------------------------------ */
/* queue_frame() */

/* No free frame: a duplicate, whatever the queue, the count and raw. */
static void steps_queue_no_free_frame(void **state)
{
	(void)state;
	const int counts[] = {INT_MIN, -1, 0, 1, 2, 3, INT_MAX};
	for (size_t c = 0; c < sizeof(counts) / sizeof(counts[0]); c++) {
		for (int raw = 0; raw < 2; raw++) {
			check_queue(0, 0, counts[c], raw, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
			check_queue(0, FRAME, counts[c], raw, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
			check_queue(0, 3 * FRAME, counts[c], raw, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
		}
	}
}

/* A free frame and one queued: a repeat (count 2 and 3) duplicates; a
 * single frame (count 1) is new, copied for raw output, else swapped. */
static void steps_queue_free_and_queued(void **state)
{
	(void)state;
	for (int raw = 0; raw < 2; raw++) {
		const enum obs_video_step_queue_action single = raw ? OBS_VIDEO_STEP_QUEUE_COPY
								    : OBS_VIDEO_STEP_QUEUE_SWAP;
		check_queue(FRAME, FRAME, 1, raw, single);
		check_queue(FRAME, FRAME, 2, raw, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
		check_queue(FRAME, FRAME, 3, raw, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
		check_queue(2 * FRAME, 3 * FRAME, 1, raw, single);
		check_queue(2 * FRAME, 3 * FRAME, 3, raw, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
	}
}

/* A free frame and none queued: always new. A repeat (count 2 and 3) is
 * copied even without raw output (the very first frame repeated); count 1
 * is copied only for raw output. */
static void steps_queue_free_and_empty(void **state)
{
	(void)state;
	check_queue(FRAME, 0, 1, false, OBS_VIDEO_STEP_QUEUE_SWAP);
	check_queue(FRAME, 0, 1, true, OBS_VIDEO_STEP_QUEUE_COPY);
	for (int count = 2; count <= 3; count++) {
		check_queue(FRAME, 0, count, false, OBS_VIDEO_STEP_QUEUE_COPY);
		check_queue(FRAME, 0, count, true, OBS_VIDEO_STEP_QUEUE_COPY);
	}
}

/* Characterized, not endorsed: counts of 0 and below are new frames like
 * count 1 (video_sleep() gives counts of 1 and more), and the sizes are only
 * tested for 0, so a size that is not a whole frame counts as a frame. */
static void steps_queue_odd_inputs(void **state)
{
	(void)state;
	check_queue(FRAME, FRAME, 0, false, OBS_VIDEO_STEP_QUEUE_SWAP);
	check_queue(FRAME, FRAME, -1, true, OBS_VIDEO_STEP_QUEUE_COPY);
	check_queue(FRAME, FRAME, INT_MIN, false, OBS_VIDEO_STEP_QUEUE_SWAP);
	check_queue(FRAME, FRAME, INT_MAX, false, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
	check_queue(1, 0, 1, false, OBS_VIDEO_STEP_QUEUE_SWAP);
	check_queue(1, 1, 2, false, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
	check_queue(SIZE_MAX, SIZE_MAX, 2, true, OBS_VIDEO_STEP_QUEUE_DUPLICATE);
	check_queue(SIZE_MAX, 0, INT_MAX, false, OBS_VIDEO_STEP_QUEUE_COPY);
}

/* The UV texture is copied on its own unless the host's NV12 planes share
 * one texture (D3D). */
static void steps_queue_copy_uv(void **state)
{
	(void)state;
	assert_true(obs_video_step_queue_copy_uv(false));
	assert_false(obs_video_step_queue_copy_uv(true));
}

/* The count left after each queue_frame(): encode_gpu() loops while it is
 * not 0, so a count of 1, 2 or 3 queues or duplicates that many frames. */
static void steps_queue_count(void **state)
{
	(void)state;
	for (int count = 1; count <= 3; count++) {
		int left = count;
		int frames = 0;
		do {
			left = obs_video_step_queue_count(left);
			frames++;
		} while (left && frames < 10);
		if (frames != count) {
			fprintf(stderr, "count %d: %d frames\n", count, frames);
			fail();
		}
	}
	assert_int_equal(obs_video_step_queue_count(1), 0);
	assert_int_equal(obs_video_step_queue_count(2), 1);
	assert_int_equal(obs_video_step_queue_count(3), 2);
	/* Characterized, not endorsed: 0 goes to -1, which encode_gpu() would
	 * keep looping on. video_sleep() gives counts of 1 and more. */
	assert_int_equal(obs_video_step_queue_count(0), -1);
	assert_int_equal(obs_video_step_queue_count(INT_MAX), INT_MAX - 1);
}

/* ------------------------------------------------------------------------ */
/* render_video() */

static void steps_render_targets(void **state)
{
	(void)state;
	for (int raw = 0; raw < 2; raw++) {
		for (int gpu = 0; gpu < 2; gpu++) {
			const struct obs_video_step_render_targets got = obs_video_step_render_targets(raw, gpu);
			const bool outputs = raw || gpu;
			if (got.outputs != outputs || got.encode != (outputs && gpu)) {
				fprintf(stderr, "targets raw %d gpu %d: outputs %d encode %d\n", raw, gpu, got.outputs,
					got.encode);
				fail();
			}
		}
	}
	/* Nothing to render without an active output. */
	const struct obs_video_step_render_targets none = obs_video_step_render_targets(false, false);
	assert_false(none.outputs);
	assert_false(none.encode);
}

/* Each combination: convert with GPU conversion, encode for GPU encoding,
 * stage for raw output. */
static void steps_render_every_combination(void **state)
{
	(void)state;
	for (int raw = 0; raw < 2; raw++) {
		for (int gpu = 0; gpu < 2; gpu++) {
			for (int conversion = 0; conversion < 2; conversion++) {
				const struct obs_video_step_render got = obs_video_step_render(raw, gpu, conversion);
				if (got.convert != (bool)conversion || got.encode != (bool)gpu ||
				    got.stage != (bool)raw) {
					fprintf(stderr,
						"render raw %d gpu %d conversion %d: convert %d encode %d stage %d\n",
						raw, gpu, conversion, got.convert, got.encode, got.stage);
					fail();
				}
			}
		}
	}
}

/* ------------------------------------------------------------------------ */
/* output_frame() */

/* The ring of NUM_TEXTURES: 0 downloads the last texture and moves to 1;
 * the last downloads 0 and wraps to 0. */
static void steps_ring_wrap(void **state)
{
	(void)state;
	check_ring(0, OBS_VIDEO_STEP_TEXTURES - 1, 1);
	check_ring(OBS_VIDEO_STEP_TEXTURES - 1, OBS_VIDEO_STEP_TEXTURES - 2, 0);

	/* Two frames go round the ring once. */
	int cur = 0;
	for (int frame = 0; frame < 2 * OBS_VIDEO_STEP_TEXTURES; frame++) {
		const struct obs_video_step_ring ring = obs_video_step_ring(cur);
		if (ring.prev != (cur + OBS_VIDEO_STEP_TEXTURES - 1) % OBS_VIDEO_STEP_TEXTURES) {
			fprintf(stderr, "frame %d: prev %d of %d\n", frame, ring.prev, cur);
			fail();
		}
		cur = ring.next;
	}
	assert_int_equal(cur, 0);
}

/* Characterized, not endorsed: outside the ring, prev is cur - 1 and next
 * is cur + 1; only exactly NUM_TEXTURES wraps. cur_texture is only ever set
 * to 0 or by the ring. */
static void steps_ring_outside(void **state)
{
	(void)state;
	check_ring(OBS_VIDEO_STEP_TEXTURES, OBS_VIDEO_STEP_TEXTURES - 1, OBS_VIDEO_STEP_TEXTURES + 1);
	check_ring(-1, -2, 0);
	check_ring(INT_MAX - 1, INT_MAX - 2, INT_MAX);
	check_ring(INT_MIN + 1, INT_MIN, INT_MIN + 2);
}

static void steps_download_and_output(void **state)
{
	(void)state;
	assert_true(obs_video_step_download(true));
	assert_false(obs_video_step_download(false));
	assert_true(obs_video_step_output(true, true));
	assert_false(obs_video_step_output(true, false));
	assert_false(obs_video_step_output(false, true));
	assert_false(obs_video_step_output(false, false));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(steps_constants),
		cmocka_unit_test(steps_activation_every_combination),
		cmocka_unit_test(steps_activation_edges),
		cmocka_unit_test(steps_activation_inconsistent_was_flags),
		cmocka_unit_test(steps_stage_without_conversion),
		cmocka_unit_test(steps_stage_with_conversion),
		cmocka_unit_test(steps_stage_not_converted),
		cmocka_unit_test(steps_stage_count_past_channels),
		cmocka_unit_test(steps_queue_no_free_frame),
		cmocka_unit_test(steps_queue_free_and_queued),
		cmocka_unit_test(steps_queue_free_and_empty),
		cmocka_unit_test(steps_queue_odd_inputs),
		cmocka_unit_test(steps_queue_copy_uv),
		cmocka_unit_test(steps_queue_count),
		cmocka_unit_test(steps_render_targets),
		cmocka_unit_test(steps_render_every_combination),
		cmocka_unit_test(steps_ring_wrap),
		cmocka_unit_test(steps_ring_outside),
		cmocka_unit_test(steps_download_and_output),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
