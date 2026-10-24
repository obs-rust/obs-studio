#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-transition-render.h"

/*
 * Characterization of the render and audio step decisions of the transitions
 * in libobs/obs-source-transition.c, moved to
 * libobs/obs-source-transition-render.c before their Rust port:
 * - obs_source_transition_render_video_due() and _video_stop(): the stop
 *   step of obs_transition_video_render2() and
 *   obs_transition_video_render_direct() (the direct render ends the audio
 *   of a transition that is not active);
 * - obs_source_transition_render_draw(): render2's draw choice;
 * - obs_source_transition_render_direct_index(): the child the direct
 *   render draws;
 * - obs_source_transition_render_audio_stop(): the stop step of
 *   obs_transition_audio_render(), with and without a timestamp;
 * - obs_source_transition_render_audio_mix(): mix both children or copy A;
 * - obs_source_transition_render_audio_valid() and _audio_place(): the
 *   checks and the start frame of transition_process_audio();
 * - obs_source_transition_render_offset(): the scaled child size and the
 *   offsets recalculate_transition_matrix() passes to add_alignment().
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_transition_swap,
 * test_studio_mode_swap, test_transition_scale_type,
 * test_transition_recursive_destination, test_item_transition_empty_source,
 * test_saved_transition, test_fade_to_color_mix,
 * test_transition_audio_rescale and test_group_hide_transition_audio cover
 * the callers through the real libobs.
 *
 * Every pinned float is the same on every platform the tests run on: the
 * offsets are one float multiply, rounded, then one subtract, as the C
 * writes them in two statements (no compiler the tests use fuses them; GCC
 * on aarch64 Linux could, which no test lane runs).
 */

#define TARGET_A 0 /* OBS_TRANSITION_SOURCE_A */
#define TARGET_B OBS_SOURCE_TRANSITION_RENDER_TARGET_B
#define FRAMES OBS_SOURCE_TRANSITION_RENDER_AUDIO_FRAMES

#define VIDEO_END OBS_SOURCE_TRANSITION_RENDER_VIDEO_END
#define VIDEO_STOP OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP
#define VIDEO_STOP_AUDIO OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO

#define DRAW_CALLBACK OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK
#define DRAW_B OBS_SOURCE_TRANSITION_RENDER_DRAW_B
#define DRAW_A OBS_SOURCE_TRANSITION_RENDER_DRAW_A

#define AUDIO_KEEP OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP
#define AUDIO_END OBS_SOURCE_TRANSITION_RENDER_AUDIO_END
#define AUDIO_STOP OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP

#define MIX_A OBS_SOURCE_TRANSITION_RENDER_MIX_A
#define MIX_B OBS_SOURCE_TRANSITION_RENDER_MIX_B
#define COPY_A OBS_SOURCE_TRANSITION_RENDER_COPY_A

#define PLACE_NONE OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE
#define PLACE_LATE OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE
#define PLACE_MIX OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX

#define SEC 1000000000ULL
#define POS_UNSET ((size_t)0x5a5a5a5a)

static uint32_t bits_of(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

static float float_of(uint32_t bits)
{
	float value;
	memcpy(&value, &bits, sizeof(value));
	return value;
}

/* t just below 1, at 1, just above 1, and NaN */
#define BELOW_ONE 0x3f7fffffu
#define ONE 0x3f800000u
#define ABOVE_ONE 0x3f800001u
#define QNAN 0x7fc00000u

/* every t the stop steps are pinned at, and whether t >= 1 holds */
static const struct {
	uint32_t t;
	bool at_least_one;
} t_cases[] = {
	{0x00000000u, false},                                                      /* 0 */
	{0x80000000u, false},                                                      /* -0 */
	{0x00000001u, false},                                                      /* the smallest subnormal */
	{0x3f000000u, false},                                                      /* 0.5 */
	{BELOW_ONE, false},   {ONE, true}, {ABOVE_ONE, true}, {0x40000000u, true}, /* 2 */
	{0x7f800000u, true},                                                       /* inf */
	{0xff800000u, false},                                                      /* -inf */
	{0xbf800000u, false},                                                      /* -1 */
	{QNAN, false},        /* characterized, not endorsed: a NaN t never ends a transition */
	{0xffc00000u, false}, /* -NaN */
	{0x7f800001u, false}, /* a signalling NaN */
};

static void video_due_at_and_around_one(void **state)
{
	(void)state;

	for (size_t i = 0; i < sizeof(t_cases) / sizeof(t_cases[0]); i++) {
		float t = float_of(t_cases[i].t);
		if (obs_source_transition_render_video_due(t, true) != t_cases[i].at_least_one) {
			fprintf(stderr, "video_due(%08x, true) is not %d\n", t_cases[i].t, t_cases[i].at_least_one);
			fail();
		}
		/* never due while the video is not transitioning */
		assert_false(obs_source_transition_render_video_due(t, false));
	}
}

static void video_stop_every_flag(void **state)
{
	(void)state;

	/* the audio runs on: only the video ends */
	assert_int_equal(obs_source_transition_render_video_stop(true, true), VIDEO_END);
	/* the audio had ended: the transition stops */
	assert_int_equal(obs_source_transition_render_video_stop(false, true), VIDEO_STOP);
	/* not active (the direct render only): the audio ends too, and the
	 * transition stops, whether or not the audio was still running */
	assert_int_equal(obs_source_transition_render_video_stop(true, false), VIDEO_STOP_AUDIO);
	assert_int_equal(obs_source_transition_render_video_stop(false, false), VIDEO_STOP_AUDIO);
}

/* The stop step as the render functions apply it under the lock. */
struct video_outcome {
	bool video;
	bool audio;
	bool video_stopped; /* "source_transition_video_stop" */
	bool stopped;       /* obs_transition_stop() and handle_stop() */
};

static struct video_outcome apply_video_step(float t, bool video, bool audio, bool direct, bool active)
{
	struct video_outcome out = {video, audio, false, false};

	if (obs_source_transition_render_video_due(t, out.video)) {
		enum obs_source_transition_render_video_step step;

		out.video = false;
		out.video_stopped = true;
		step = obs_source_transition_render_video_stop(out.audio, direct ? active : true);
		if (step == VIDEO_STOP_AUDIO)
			out.audio = false;
		if (step != VIDEO_END)
			out.stopped = true;
	}
	return out;
}

static void check_video_outcome(const char *what, uint32_t t, int flags, struct video_outcome got,
				struct video_outcome want)
{
	if (got.video != want.video || got.audio != want.audio || got.video_stopped != want.video_stopped ||
	    got.stopped != want.stopped) {
		fprintf(stderr, "%s t=%08x flags=%d: got v=%d a=%d vs=%d s=%d, want v=%d a=%d vs=%d s=%d\n", what, t,
			flags, got.video, got.audio, got.video_stopped, got.stopped, want.video, want.audio,
			want.video_stopped, want.stopped);
		fail();
	}
}

static void render2_stop_step_combinations(void **state)
{
	static const uint32_t ts[] = {BELOW_ONE, ONE, ABOVE_ONE, QNAN};
	(void)state;

	for (size_t i = 0; i < 4; i++) {
		bool due_t = ts[i] == ONE || ts[i] == ABOVE_ONE;
		for (int flags = 0; flags < 4; flags++) {
			bool video = flags & 1;
			bool audio = flags & 2;
			struct video_outcome want = {video, audio, false, false};

			if (due_t && video) {
				want.video = false;
				want.video_stopped = true;
				want.stopped = !audio;
			}
			check_video_outcome("render2", ts[i], flags,
					    apply_video_step(float_of(ts[i]), video, audio, false, false), want);
		}
	}
}

static void direct_stop_step_combinations(void **state)
{
	static const uint32_t ts[] = {BELOW_ONE, ONE, ABOVE_ONE, QNAN};
	(void)state;

	for (size_t i = 0; i < 4; i++) {
		bool due_t = ts[i] == ONE || ts[i] == ABOVE_ONE;
		for (int flags = 0; flags < 8; flags++) {
			bool video = flags & 1;
			bool audio = flags & 2;
			bool active = flags & 4;
			struct video_outcome want = {video, audio, false, false};

			if (due_t && video) {
				want.video = false;
				want.video_stopped = true;
				if (!active)
					want.audio = false;
				want.stopped = !want.audio;
			}
			check_video_outcome("direct", ts[i], flags,
					    apply_video_step(float_of(ts[i]), video, audio, true, active), want);
		}
	}
}

static void draw_every_flag(void **state)
{
	(void)state;

	for (int flags = 0; flags < 16; flags++) {
		bool video = flags & 1;
		bool locked = flags & 2;
		bool callback = flags & 4;
		bool audio = flags & 8;
		int want;

		if (video && locked && callback)
			want = DRAW_CALLBACK;
		else if (audio)
			want = DRAW_B;
		else
			want = DRAW_A;
		if ((int)obs_source_transition_render_draw(video, locked, callback, audio) != want) {
			fprintf(stderr, "draw(%d, %d, %d, %d) is not %d\n", video, locked, callback, audio, want);
			fail();
		}
	}

	/* characterized, not endorsed: a video transition whose textures are
	 * busy, or that has no callback, draws B only while its audio also
	 * transitions, and source A (the old scene) otherwise */
	assert_int_equal(obs_source_transition_render_draw(true, false, true, false), DRAW_A);
	assert_int_equal(obs_source_transition_render_draw(true, true, false, false), DRAW_A);
	assert_int_equal(obs_source_transition_render_draw(true, false, true, true), DRAW_B);
}

static void direct_index_every_flag(void **state)
{
	(void)state;

	assert_int_equal(obs_source_transition_render_direct_index(false, TARGET_A), 0);
	assert_int_equal(obs_source_transition_render_direct_index(false, TARGET_B), 0);
	assert_int_equal(obs_source_transition_render_direct_index(true, TARGET_A), 0);
	assert_int_equal(obs_source_transition_render_direct_index(true, TARGET_B), 1);

	/* targets outside the enum draw A */
	static const int others[] = {2, -1, 0x7fffffff, (int)0x80000000u};
	for (size_t i = 0; i < 4; i++) {
		assert_int_equal(obs_source_transition_render_direct_index(true, others[i]), 0);
		assert_int_equal(obs_source_transition_render_direct_index(false, others[i]), 0);
	}
}

static void audio_stop_with_timestamp(void **state)
{
	(void)state;

	for (size_t i = 0; i < sizeof(t_cases) / sizeof(t_cases[0]); i++) {
		float t = float_of(t_cases[i].t);
		for (int flags = 0; flags < 4; flags++) {
			bool video = flags & 1;
			bool audio = flags & 2;
			int want = AUDIO_KEEP;

			if (t_cases[i].at_least_one && audio)
				want = video ? AUDIO_END : AUDIO_STOP;
			if ((int)obs_source_transition_render_audio_stop(true, t, video, audio) != want) {
				fprintf(stderr, "audio_stop(true, %08x, %d, %d) is not %d\n", t_cases[i].t, video,
					audio, want);
				fail();
			}
		}
	}
}

static void audio_stop_without_timestamp(void **state)
{
	(void)state;

	/* No child gave a timestamp: the audio ends only once the video has
	 * ended (7af9c2d88), so a child whose audio starts late still joins.
	 * t is not read: calc_time() is not asked without a timestamp. */
	for (size_t i = 0; i < sizeof(t_cases) / sizeof(t_cases[0]); i++) {
		float t = float_of(t_cases[i].t);
		assert_int_equal(obs_source_transition_render_audio_stop(false, t, false, false), AUDIO_KEEP);
		assert_int_equal(obs_source_transition_render_audio_stop(false, t, false, true), AUDIO_STOP);
		assert_int_equal(obs_source_transition_render_audio_stop(false, t, true, false), AUDIO_KEEP);
		assert_int_equal(obs_source_transition_render_audio_stop(false, t, true, true), AUDIO_KEEP);
	}
}

/* What obs_transition_audio_render() passes for one child: a child that is
 * not there or removed (obs_source_removed()) is dropped before the
 * timestamps are read, and one whose audio is pending passes 0, as one whose
 * timestamp is 0 does (transition_calc_min_ts()). */
enum child_state { ABSENT, REMOVED, PENDING, NO_TS, HAS_TS };

static bool gives_timestamp(enum child_state s)
{
	return s == HAS_TS;
}

static void audio_step_children(void **state)
{
	(void)state;

	for (int a = ABSENT; a <= HAS_TS; a++) {
		for (int b = ABSENT; b <= HAS_TS; b++) {
			bool has_ts = gives_timestamp((enum child_state)a) || gives_timestamp((enum child_state)b);
			for (int flags = 0; flags < 4; flags++) {
				bool video = flags & 1;
				bool audio = flags & 2;
				int want = AUDIO_KEEP;

				/* at t = 1 */
				if (audio && (has_ts || !video))
					want = video ? AUDIO_END : AUDIO_STOP;
				if ((int)obs_source_transition_render_audio_stop(has_ts, 1.0f, video, audio) != want) {
					fprintf(stderr, "children %d %d flags %d: not %d\n", a, b, flags, want);
					fail();
				}
				/* just below 1: only the no-timestamp rule ends the audio */
				want = (!has_ts && audio && !video) ? AUDIO_STOP : AUDIO_KEEP;
				if ((int)obs_source_transition_render_audio_stop(has_ts, float_of(BELOW_ONE), video,
										 audio) != want) {
					fprintf(stderr, "children %d %d flags %d below 1: not %d\n", a, b, flags, want);
					fail();
				}
			}
		}
	}

	/* the copied children after the re-read: each one there is mixed while
	 * the audio transitions; a removed, pending or duplicated one is still
	 * passed to transition_process_audio(), which then reads nothing
	 * (characterized, not endorsed: the re-read does not drop removed
	 * children; transition_process_audio() checks only pending and
	 * duplicated) */
	assert_int_equal(obs_source_transition_render_audio_mix(true, true, true), MIX_A | MIX_B);
	assert_false(obs_source_transition_render_audio_valid(true, true, false));
}

static void audio_mix_every_flag(void **state)
{
	(void)state;

	assert_int_equal(obs_source_transition_render_audio_mix(false, false, false), 0);
	assert_int_equal(obs_source_transition_render_audio_mix(false, true, false), COPY_A);
	assert_int_equal(obs_source_transition_render_audio_mix(false, false, true), 0);
	assert_int_equal(obs_source_transition_render_audio_mix(false, true, true), COPY_A);
	assert_int_equal(obs_source_transition_render_audio_mix(true, false, false), 0);
	assert_int_equal(obs_source_transition_render_audio_mix(true, true, false), MIX_A);
	assert_int_equal(obs_source_transition_render_audio_mix(true, false, true), MIX_B);
	assert_int_equal(obs_source_transition_render_audio_mix(true, true, true), MIX_A | MIX_B);
}

static void audio_valid_every_flag(void **state)
{
	(void)state;

	for (int flags = 0; flags < 8; flags++) {
		bool present = flags & 1;
		bool pending = flags & 2;
		bool duplicated = flags & 4;
		bool want = present && !pending && !duplicated;
		if (obs_source_transition_render_audio_valid(present, pending, duplicated) != want) {
			fprintf(stderr, "audio_valid(%d, %d, %d) is not %d\n", present, pending, duplicated, want);
			fail();
		}
	}
}

static void check_place(uint64_t ts, uint64_t min_ts, size_t rate, int want, size_t want_pos)
{
	size_t pos = POS_UNSET;
	int got = (int)obs_source_transition_render_audio_place(ts, min_ts, rate, &pos);

	if (got != want || pos != want_pos) {
		fprintf(stderr, "audio_place(%llu, %llu, %zu): got %d pos %zu, want %d pos %zu\n",
			(unsigned long long)ts, (unsigned long long)min_ts, rate, got, pos, want, want_pos);
		fail();
	}
}

static void audio_place_no_timestamp(void **state)
{
	(void)state;

	/* a timestamp of 0 (3191f5ddb): nothing more is read, pos untouched */
	check_place(0, 0, 48000, PLACE_NONE, POS_UNSET);
	check_place(0, 5 * SEC, 48000, PLACE_NONE, POS_UNSET);
	check_place(0, UINT64_MAX, 44100, PLACE_NONE, POS_UNSET);
}

static void audio_place_around_min_ts(void **state)
{
	const uint64_t min_ts = 5 * SEC;
	(void)state;

	/* at 48 kHz a frame is 20833.3 ns: frame k starts at ceil(k * 1e9 / 48000) */
	check_place(min_ts, min_ts, 48000, PLACE_MIX, 0);
	check_place(min_ts + 1, min_ts, 48000, PLACE_MIX, 0);
	check_place(min_ts + 20833, min_ts, 48000, PLACE_MIX, 0);
	check_place(min_ts + 20834, min_ts, 48000, PLACE_MIX, 1);
	check_place(min_ts + 21312499, min_ts, 48000, PLACE_MIX, 1022);
	check_place(min_ts + 21312500, min_ts, 48000, PLACE_MIX, 1023);
	check_place(min_ts + 21333333, min_ts, 48000, PLACE_MIX, 1023);
	/* characterized, not endorsed: a start at AUDIO_OUTPUT_FRAMES is not
	 * late (the scene's window uses >=); mix_child() then mixes 0 frames */
	check_place(min_ts + 21333334, min_ts, 48000, PLACE_MIX, FRAMES);
	check_place(min_ts + 21354166, min_ts, 48000, PLACE_MIX, FRAMES);
	check_place(min_ts + 21354167, min_ts, 48000, PLACE_LATE, FRAMES + 1);
	check_place(min_ts + SEC, min_ts, 48000, PLACE_LATE, 48000);

	/* 44.1 kHz: frame 1024 starts at 23219955 ns, frame 1025 at 23242631 */
	check_place(min_ts + 23219954, min_ts, 44100, PLACE_MIX, 1023);
	check_place(min_ts + 23219955, min_ts, 44100, PLACE_MIX, FRAMES);
	check_place(min_ts + 23242630, min_ts, 44100, PLACE_MIX, FRAMES);
	check_place(min_ts + 23242631, min_ts, 44100, PLACE_LATE, FRAMES + 1);

	/* a sample rate of 0 places everything at 0; 1e9 places by the ns */
	check_place(min_ts + 7 * SEC, min_ts, 0, PLACE_MIX, 0);
	check_place(min_ts + 1024, min_ts, 1000000000, PLACE_MIX, 1024);
	check_place(min_ts + 1025, min_ts, 1000000000, PLACE_LATE, 1025);
	check_place(min_ts + SEC, min_ts, 1, PLACE_MIX, 1);
}

static void audio_place_u64_wrap(void **state)
{
	(void)state;

	/* characterized, not endorsed: a timestamp below min_ts wraps in
	 * uint64_t; with a 64-bit size_t the frame is far past the tick, so it
	 * is late (read, nothing mixed) */
	if (sizeof(size_t) == 8) {
		/* (2^64 - 1) * 48000 / 1e9, the libobs formula */
		check_place(5 * SEC - 1, 5 * SEC, 48000, PLACE_LATE, (size_t)885443715538058ULL);
	}
	/* a min_ts past UINT64_MAX's wrap: ts - min_ts lands near 0 again */
	check_place(1, UINT64_MAX, 48000, PLACE_MIX, 0);
	check_place(20834, UINT64_MAX, 48000, PLACE_MIX, 1);
	check_place(UINT64_MAX, UINT64_MAX, 48000, PLACE_MIX, 0);
	check_place(UINT64_MAX, 1, 48000, PLACE_LATE, (size_t)885443715538058ULL);
}

static void check_offset(float tr_cx, float tr_cy, float source_cx, float source_cy, float scale_x, float scale_y,
			 uint32_t want_x, uint32_t want_y)
{
	struct obs_source_transition_render_offset got =
		obs_source_transition_render_offset(tr_cx, tr_cy, source_cx, source_cy, scale_x, scale_y);

	if (bits_of(got.x) != want_x || bits_of(got.y) != want_y) {
		fprintf(stderr, "offset(%g, %g, %g, %g, %08x, %08x): got %08x %08x, want %08x %08x\n", tr_cx, tr_cy,
			source_cx, source_cy, bits_of(scale_x), bits_of(scale_y), bits_of(got.x), bits_of(got.y),
			want_x, want_y);
		fail();
	}
}

static void offset_each_scale_type(void **state)
{
	(void)state;

	/* 1280x720 in 1920x1080: max-only keeps 1, aspect and stretch scale 1.5 */
	check_offset(1920, 1080, 1280, 720, 1.0f, 1.0f, bits_of(640.0f), bits_of(360.0f));
	check_offset(1920, 1080, 1280, 720, 1.5f, 1.5f, bits_of(0.0f), bits_of(0.0f));

	/* 1000x1000 in 1920x1080: aspect fits the height, stretch both sides */
	check_offset(1920, 1080, 1000, 1000, 1.08f, 1.08f, bits_of(840.0f), bits_of(0.0f));
	check_offset(1920, 1080, 1000, 1000, 1.92f, 1.08f, bits_of(0.0f), bits_of(0.0f));

	/* 3840x2160 in 1920x1080 (max-only fits a larger child as aspect) */
	check_offset(1920, 1080, 3840, 2160, 0.5f, 0.5f, bits_of(0.0f), bits_of(0.0f));
	/* a larger child at a scale of 1, the scale's initial value */
	check_offset(1920, 1080, 3840, 2160, 1.0f, 1.0f, bits_of(-1920.0f), bits_of(-1080.0f));

	/* 32x16 in 64x64 (test_transition_scale_type's case) */
	check_offset(64, 64, 32, 16, 1.0f, 1.0f, bits_of(32.0f), bits_of(48.0f));
	check_offset(64, 64, 32, 16, 2.0f, 2.0f, bits_of(0.0f), bits_of(32.0f));
	check_offset(64, 64, 32, 16, 2.0f, 4.0f, bits_of(0.0f), bits_of(0.0f));
}

static void offset_rounding_and_specials(void **state)
{
	(void)state;

	/* the product is rounded to float before the subtract: 3 * 0x3eaaaaab
	 * is 1.00000002, which rounds to 1, so 1 - 1 is exactly 0 (a fused
	 * multiply-subtract would give -2^-25) */
	check_offset(1, 1, 3, 3, float_of(0x3eaaaaabu), float_of(0x3eaaaaabu), 0x00000000u, 0x00000000u);
	/* 1053 * (1080 / 1053 as float) rounds to 1080.0001: the offset is
	 * -2^-13, just below 0 (the (int) cast in the C makes it 0) */
	check_offset(1080, 1080, 1053, 1053, float_of(0x3f834835u), float_of(0x3f834835u), 0xb9000000u, 0xb9000000u);

	/* a transition of size 0 (scale 0 from the fit) */
	check_offset(0, 0, 100, 100, 0.0f, 0.0f, bits_of(0.0f), bits_of(0.0f));
	/* characterized, not endorsed: an infinite scale (from a transition
	 * of size 0 on the other side) gives -inf, and inf - inf is NaN */
	check_offset(1920, 1080, 100, 100, INFINITY, INFINITY, 0xff800000u, 0xff800000u);
	{
		struct obs_source_transition_render_offset got =
			obs_source_transition_render_offset(INFINITY, 1, 100, 1, INFINITY, 1.0f);
		assert_true(isnan(got.x));
		assert_int_equal(bits_of(got.y), bits_of(0.0f));
	}
	/* a NaN scale is a NaN offset */
	{
		struct obs_source_transition_render_offset got =
			obs_source_transition_render_offset(1920, 1080, 100, 100, NAN, 1.0f);
		assert_true(isnan(got.x));
		assert_int_equal(bits_of(got.y), bits_of(980.0f));
	}
	/* the largest sizes: a u32 size is 4294967296.0f as float */
	check_offset(4294967296.0f, 1, 1, 1, 1.0f, 1.0f, bits_of(4294967296.0f), bits_of(0.0f));
	check_offset(1, 1, 4294967296.0f, 1, 1.0f, 1.0f, bits_of(-4294967296.0f), bits_of(0.0f));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(video_due_at_and_around_one),
		cmocka_unit_test(video_stop_every_flag),
		cmocka_unit_test(render2_stop_step_combinations),
		cmocka_unit_test(direct_stop_step_combinations),
		cmocka_unit_test(draw_every_flag),
		cmocka_unit_test(direct_index_every_flag),
		cmocka_unit_test(audio_stop_with_timestamp),
		cmocka_unit_test(audio_stop_without_timestamp),
		cmocka_unit_test(audio_step_children),
		cmocka_unit_test(audio_mix_every_flag),
		cmocka_unit_test(audio_valid_every_flag),
		cmocka_unit_test(audio_place_no_timestamp),
		cmocka_unit_test(audio_place_around_min_ts),
		cmocka_unit_test(audio_place_u64_wrap),
		cmocka_unit_test(offset_each_scale_type),
		cmocka_unit_test(offset_rounding_and_specials),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
