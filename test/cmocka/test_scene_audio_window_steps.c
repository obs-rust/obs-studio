#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <inttypes.h>
#include <setjmp.h>
#include <cmocka.h>

#include "media-io/audio-io.h"
#include "obs-scene-audio-window.h"

/*
 * Characterization of the audio window decisions of scene_audio_render() in
 * libobs/obs-scene.c, moved to libobs/obs-scene-audio-window.c before their
 * Rust port:
 * - the source an item's audio is read from: the show transition while it
 *   plays for a visible item, the hide transition for a hidden one, else the
 *   source;
 * - the tick's timestamp: the earliest over the items that are not pending
 *   and are visible or hiding, 0 when there is none (the scene then only
 *   processes its items' pending audio actions and reports no audio);
 * - each item's window: skipped when pending, with no timestamp, or from
 *   frame AUDIO_OUTPUT_FRAMES on; otherwise frames pos to the end of the
 *   tick, pos = ns_to_audio_frames(rate, source_ts - timestamp) in uint64_t;
 * - a hidden item with no volume buffer and no hide transition is not mixed;
 * - duplicated audio is read but not added.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them (test_scene_audio_window_steps_rust runs
 * it against the Rust shims). test_group_hide_transition_audio,
 * test_scene_audio_actions, test_visibility_transition,
 * test_preview_only_hide and test_audio_capture_mix_race cover the caller
 * through the real libobs.
 */

#define FRAMES AUDIO_OUTPUT_FRAMES
/* a tick timestamp well away from 0, which the C treats as "no timestamp" */
#define TICK 1000000000000ULL

/* util_mul_div64() is the wrapping uint64_t formula everywhere but MSVC
 * x64, where it is exact 128-bit math that faults when the quotient does
 * not fit (rates above 1 GHz). */
#if defined(_MSC_VER) && defined(_M_X64) && !defined(__clang__)
#define MUL_DIV64_WRAPS 0
#else
#define MUL_DIV64_WRAPS 1
#endif

static void expect_u64(uint64_t got, uint64_t want, const char *what)
{
	if (got != want) {
		fprintf(stderr, "%s: got %" PRIu64 ", expected %" PRIu64 "\n", what, got, want);
		fail();
	}
}

static void expect_bool(bool got, bool want, const char *what)
{
	if (got != want) {
		fprintf(stderr, "%s: got %d, expected %d\n", what, got, want);
		fail();
	}
}

/* frames() gives (mix, pos, count) */
static void expect_window(struct obs_scene_audio_window w, bool mix, size_t pos, const char *what)
{
	if (w.mix != mix || w.pos != pos || w.count != (mix ? FRAMES - pos : 0)) {
		fprintf(stderr, "%s: got mix %d pos %zu count %zu, expected mix %d pos %zu count %zu\n", what, w.mix,
			w.pos, w.count, mix, pos, mix ? (size_t)FRAMES - pos : (size_t)0);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* The source read                                                           */

static void pick_every_combination(void **state)
{
	(void)state;

	/* visible, show active, hide active -> child */
	static const struct {
		bool visible, show, hide;
		enum obs_scene_audio_window_child child;
	} cases[] = {
		{false, false, false, OBS_SCENE_AUDIO_WINDOW_SOURCE},
		{false, false, true, OBS_SCENE_AUDIO_WINDOW_HIDE},
		/* a show transition of a hidden item is not read */
		{false, true, false, OBS_SCENE_AUDIO_WINDOW_SOURCE},
		{false, true, true, OBS_SCENE_AUDIO_WINDOW_HIDE},
		{true, false, false, OBS_SCENE_AUDIO_WINDOW_SOURCE},
		/* a hide transition of a visible item is not read */
		{true, false, true, OBS_SCENE_AUDIO_WINDOW_SOURCE},
		{true, true, false, OBS_SCENE_AUDIO_WINDOW_SHOW},
		{true, true, true, OBS_SCENE_AUDIO_WINDOW_SHOW},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		enum obs_scene_audio_window_child got =
			obs_scene_audio_window_pick(cases[i].visible, cases[i].show, cases[i].hide);
		if (got != cases[i].child) {
			fprintf(stderr, "pick(%d, %d, %d) = %d, expected %d\n", cases[i].visible, cases[i].show,
				cases[i].hide, (int)got, (int)cases[i].child);
			fail();
		}
	}
}

static void child_values(void **state)
{
	(void)state;

	assert_int_equal(OBS_SCENE_AUDIO_WINDOW_SOURCE, 0);
	assert_int_equal(OBS_SCENE_AUDIO_WINDOW_SHOW, 1);
	assert_int_equal(OBS_SCENE_AUDIO_WINDOW_HIDE, 2);
	assert_int_equal(sizeof(enum obs_scene_audio_window_child), sizeof(int));
	assert_int_equal(FRAMES, 1024);
}

/* ------------------------------------------------------------------------ */
/* The earliest timestamp                                                    */

static void counts_every_combination(void **state)
{
	(void)state;

	for (int bits = 0; bits < 8; bits++) {
		const bool pending = bits & 1;
		const bool visible = bits & 2;
		const bool hiding = bits & 4;
		expect_bool(obs_scene_audio_window_counts(pending, visible, hiding), !pending && (visible || hiding),
			    "counts");
	}
}

struct timed_item {
	bool pending;
	bool visible;
	bool hiding;
	uint64_t ts;
};

/* the first loop of scene_audio_render() over the items */
static uint64_t earliest_of(const struct timed_item *items, size_t n)
{
	uint64_t timestamp = 0;
	for (size_t i = 0; i < n; i++) {
		if (obs_scene_audio_window_counts(items[i].pending, items[i].visible, items[i].hiding))
			timestamp = obs_scene_audio_window_earliest(timestamp, items[i].ts);
	}
	return timestamp;
}

static void earliest_over_item_sets(void **state)
{
	(void)state;

	/* no items */
	expect_u64(earliest_of(NULL, 0), 0, "no items");

	/* one item */
	const struct timed_item one[] = {{false, true, false, TICK}};
	expect_u64(earliest_of(one, 1), TICK, "one visible item");
	const struct timed_item one_zero[] = {{false, true, false, 0}};
	expect_u64(earliest_of(one_zero, 1), 0, "one item with no timestamp");
	const struct timed_item one_hiding[] = {{false, false, true, TICK}};
	expect_u64(earliest_of(one_hiding, 1), TICK, "one hiding item");
	const struct timed_item one_hidden[] = {{false, false, false, TICK}};
	expect_u64(earliest_of(one_hidden, 1), 0, "one hidden item, no hide transition");
	const struct timed_item one_pending[] = {{true, true, false, TICK}};
	expect_u64(earliest_of(one_pending, 1), 0, "one pending item");

	/* three items, in every order */
	const uint64_t three[3] = {TICK + 7, TICK + 3, TICK + 9};
	const int orders[6][3] = {{0, 1, 2}, {0, 2, 1}, {1, 0, 2}, {1, 2, 0}, {2, 0, 1}, {2, 1, 0}};
	for (int o = 0; o < 6; o++) {
		struct timed_item items[3];
		for (int i = 0; i < 3; i++)
			items[i] = (struct timed_item){false, true, false, three[orders[o][i]]};
		expect_u64(earliest_of(items, 3), TICK + 3, "three items");
	}

	/* items with no timestamp do not count, wherever they are */
	const struct timed_item zeros[] = {{false, true, false, 0},
					   {false, true, false, TICK + 4},
					   {false, true, false, 0}};
	expect_u64(earliest_of(zeros, 3), TICK + 4, "zeros around one timestamp");
	const struct timed_item all_zero[] = {{false, true, false, 0}, {false, false, true, 0}, {false, true, true, 0}};
	expect_u64(earliest_of(all_zero, 3), 0, "no timestamps");

	/* all pending: no audio, whatever the timestamps */
	const struct timed_item all_pending[] = {{true, true, false, TICK},
						 {true, false, true, TICK + 1},
						 {true, true, true, 1}};
	expect_u64(earliest_of(all_pending, 3), 0, "all pending");

	/* a pending or hidden item does not lower the minimum */
	const struct timed_item excluded[] = {{false, true, false, TICK + 5},
					      {true, true, false, TICK + 1},
					      {false, false, false, TICK + 2},
					      {false, false, true, TICK + 4}};
	expect_u64(earliest_of(excluded, 4), TICK + 4, "pending and hidden items excluded");

	/* equal timestamps */
	const struct timed_item equal[] = {{false, true, false, TICK}, {false, true, false, TICK}};
	expect_u64(earliest_of(equal, 2), TICK, "equal timestamps");

	/* the uint64_t edges: 1 is the smallest timestamp, UINT64_MAX the largest */
	const struct timed_item edges[] = {{false, true, false, UINT64_MAX},
					   {false, true, false, 1},
					   {false, true, false, UINT64_MAX - 1}};
	expect_u64(earliest_of(edges, 3), 1, "1 and UINT64_MAX");
	const struct timed_item max_only[] = {{false, true, false, UINT64_MAX}};
	expect_u64(earliest_of(max_only, 1), UINT64_MAX, "UINT64_MAX alone");
}

static void earliest_step(void **state)
{
	(void)state;

	expect_u64(obs_scene_audio_window_earliest(0, 0), 0, "0, 0");
	expect_u64(obs_scene_audio_window_earliest(0, 5), 5, "first timestamp");
	expect_u64(obs_scene_audio_window_earliest(5, 0), 5, "no timestamp keeps it");
	expect_u64(obs_scene_audio_window_earliest(5, 4), 4, "earlier");
	expect_u64(obs_scene_audio_window_earliest(5, 5), 5, "equal");
	expect_u64(obs_scene_audio_window_earliest(5, 6), 5, "later");
	expect_u64(obs_scene_audio_window_earliest(UINT64_MAX, 1), 1, "1 under UINT64_MAX");
	expect_u64(obs_scene_audio_window_earliest(1, UINT64_MAX), 1, "UINT64_MAX over 1");
	expect_u64(obs_scene_audio_window_earliest(0, UINT64_MAX), UINT64_MAX, "UINT64_MAX first");
}

static void silent(void **state)
{
	(void)state;

	expect_bool(obs_scene_audio_window_silent(0), true, "0");
	expect_bool(obs_scene_audio_window_silent(1), false, "1");
	expect_bool(obs_scene_audio_window_silent(TICK), false, "TICK");
	expect_bool(obs_scene_audio_window_silent(UINT64_MAX), false, "UINT64_MAX");
}

/* ------------------------------------------------------------------------ */
/* The item window                                                           */

/* At 48 kHz frame f starts at f * 1e9 / 48000 ns: frame 1023 at exactly
 * 21312500 ns, frame 1024 at 21333333.3 ns, frame 1025 at 21354166.7 ns. */
static void window_edges_48k(void **state)
{
	(void)state;

	const size_t rate = 48000;
	expect_window(obs_scene_audio_window_frames(false, TICK, TICK, rate), true, 0, "at the tick");
	expect_window(obs_scene_audio_window_frames(false, TICK + 1, TICK, rate), true, 0, "1 ns in");
	expect_window(obs_scene_audio_window_frames(false, TICK + 20833, TICK, rate), true, 0, "end of frame 0");
	expect_window(obs_scene_audio_window_frames(false, TICK + 20834, TICK, rate), true, 1, "frame 1");
	expect_window(obs_scene_audio_window_frames(false, TICK + 21312499, TICK, rate), true, 1022, "end of 1022");
	/* pos = AUDIO_OUTPUT_FRAMES - 1: one frame mixed */
	expect_window(obs_scene_audio_window_frames(false, TICK + 21312500, TICK, rate), true, 1023, "frame 1023");
	expect_window(obs_scene_audio_window_frames(false, TICK + 21333333, TICK, rate), true, 1023, "end of 1023");
	/* pos = AUDIO_OUTPUT_FRAMES: skipped */
	expect_window(obs_scene_audio_window_frames(false, TICK + 21333334, TICK, rate), false, 0, "frame 1024");
	expect_window(obs_scene_audio_window_frames(false, TICK + 21354166, TICK, rate), false, 0, "end of 1024");
	/* pos = AUDIO_OUTPUT_FRAMES + 1 */
	expect_window(obs_scene_audio_window_frames(false, TICK + 21354167, TICK, rate), false, 0, "frame 1025");
	expect_window(obs_scene_audio_window_frames(false, TICK + 1000000000, TICK, rate), false, 0, "a second on");
}

/* At 44.1 kHz, frame 1023 starts at 23197278.9 ns and frame 1024 at
 * 23219954.6 ns. */
static void window_edges_44k1(void **state)
{
	(void)state;

	const size_t rate = 44100;
	expect_window(obs_scene_audio_window_frames(false, TICK + 23197278, TICK, rate), true, 1022, "end of 1022");
	expect_window(obs_scene_audio_window_frames(false, TICK + 23197279, TICK, rate), true, 1023, "frame 1023");
	expect_window(obs_scene_audio_window_frames(false, TICK + 23219954, TICK, rate), true, 1023, "end of 1023");
	expect_window(obs_scene_audio_window_frames(false, TICK + 23219955, TICK, rate), false, 0, "frame 1024");
	expect_window(obs_scene_audio_window_frames(false, TICK + 23242631, TICK, rate), false, 0, "frame 1025");
}

/* The other rates OBS offers, and the ends of the range: frame 1023 is in,
 * frame 1024 is out. */
static void window_edges_other_rates(void **state)
{
	(void)state;

	/* rate, first ns of frame 1023, first ns of frame 1024 */
	static const struct {
		size_t rate;
		uint64_t f1023, f1024;
	} cases[] = {
		{8000, 127875000, 128000000}, {16000, 63937500, 64000000}, {22050, 46394558, 46439910},
		{32000, 31968750, 32000000},  {88200, 11598640, 11609978}, {96000, 10656250, 10666667},
		{192000, 5328125, 5333334},   {384000, 2664063, 2666667},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		const size_t rate = cases[i].rate;
		expect_window(obs_scene_audio_window_frames(false, TICK + cases[i].f1023 - 1, TICK, rate), true, 1022,
			      "end of 1022");
		expect_window(obs_scene_audio_window_frames(false, TICK + cases[i].f1023, TICK, rate), true, 1023,
			      "frame 1023");
		expect_window(obs_scene_audio_window_frames(false, TICK + cases[i].f1024 - 1, TICK, rate), true, 1023,
			      "end of 1023");
		expect_window(obs_scene_audio_window_frames(false, TICK + cases[i].f1024, TICK, rate), false, 0,
			      "frame 1024");
	}
}

static void window_skips(void **state)
{
	(void)state;

	/* pending: skipped, whatever the timestamp (the caller does not ask
	 * for it and passes 0) */
	expect_window(obs_scene_audio_window_frames(true, 0, TICK, 48000), false, 0, "pending");
	expect_window(obs_scene_audio_window_frames(true, TICK, TICK, 48000), false, 0, "pending at the tick");
	/* no timestamp */
	expect_window(obs_scene_audio_window_frames(false, 0, TICK, 48000), false, 0, "no timestamp");
	expect_window(obs_scene_audio_window_frames(false, 0, 1, 48000), false, 0, "no timestamp, tick 1");
}

/* An item left out of the minimum (hidden with no hide transition, or
 * pending on the first pass) can start before it: source_ts - timestamp
 * wraps in uint64_t to a frame far past the tick, so the item is skipped,
 * not mixed from frame 0. Characterized, not endorsed. */
static void window_before_the_minimum(void **state)
{
	(void)state;

	static const size_t rates[] = {8000, 44100, 48000, 96000, 384000};
	for (size_t i = 0; i < sizeof(rates) / sizeof(rates[0]); i++) {
		expect_window(obs_scene_audio_window_frames(false, TICK - 1, TICK, rates[i]), false, 0, "1 ns before");
		expect_window(obs_scene_audio_window_frames(false, TICK - 21333333, TICK, rates[i]), false, 0,
			      "a tick before");
		expect_window(obs_scene_audio_window_frames(false, 1, TICK, rates[i]), false, 0, "at 1");
	}

	/* Across the end of uint64_t the wrap lands back in the tick: 1 is 2 ns
	 * after UINT64_MAX - 1 and is mixed from frame 0. Characterized, not
	 * endorsed: timestamps are ns of the monotonic clock. */
	expect_window(obs_scene_audio_window_frames(false, 1, UINT64_MAX - 1, 48000), true, 0,
		      "1 after UINT64_MAX - 1");
	expect_window(obs_scene_audio_window_frames(false, 21312499, UINT64_MAX - 1, 48000), true, 1023,
		      "frame 1023 across the end");
	expect_window(obs_scene_audio_window_frames(false, 21333332, UINT64_MAX - 1, 48000), false, 0,
		      "frame 1024 across the end");
}

/* The uint64_t edges. */
static void window_u64_edges(void **state)
{
	(void)state;

	expect_window(obs_scene_audio_window_frames(false, UINT64_MAX, UINT64_MAX, 48000), true, 0, "UINT64_MAX both");
	expect_window(obs_scene_audio_window_frames(false, UINT64_MAX, UINT64_MAX - 21312500, 48000), true, 1023,
		      "frame 1023 below UINT64_MAX");
	expect_window(obs_scene_audio_window_frames(false, UINT64_MAX, UINT64_MAX - 21333334, 48000), false, 0,
		      "frame 1024 below UINT64_MAX");
	expect_window(obs_scene_audio_window_frames(false, UINT64_MAX, 1, 48000), false, 0, "1 to UINT64_MAX");
	expect_window(obs_scene_audio_window_frames(false, 2, 1, 48000), true, 0, "1 ns from 1");

	/* rate 0: every item on frame 0. Characterized, not endorsed. */
	expect_window(obs_scene_audio_window_frames(false, UINT64_MAX, 1, 0), true, 0, "rate 0");
	expect_window(obs_scene_audio_window_frames(false, 1, UINT64_MAX, 0), true, 0, "rate 0, before");
	/* rate 1: one frame a second */
	expect_window(obs_scene_audio_window_frames(false, TICK + 1023999999999ULL, TICK, 1), true, 1023, "rate 1");
	expect_window(obs_scene_audio_window_frames(false, TICK + 1024000000000ULL, TICK, 1), false, 0, "rate 1, 1024");

#if MUL_DIV64_WRAPS
	/* Rates of 1 GHz and up: (ns / 1e9) * rate wraps. 2 s at 2^63 Hz is
	 * 2^64 frames, which wraps to frame 0 and is mixed. Characterized, not
	 * endorsed: no audio device runs at these rates. */
	expect_window(obs_scene_audio_window_frames(false, TICK + 2000000000ULL, TICK, (size_t)1 << 63), true, 0,
		      "2^64 frames wrap to 0");
	expect_window(obs_scene_audio_window_frames(false, TICK + 1000000000ULL, TICK, (size_t)1 << 63), false, 0,
		      "2^63 frames");
	expect_window(obs_scene_audio_window_frames(false, TICK + 1, TICK, SIZE_MAX), false, 0, "1 ns at SIZE_MAX");
	expect_window(obs_scene_audio_window_frames(false, TICK, TICK, SIZE_MAX), true, 0, "0 ns at SIZE_MAX");
	/* (ns % 1e9) * rate wraps too: 999999999 ns at 2^62 Hz */
	expect_window(obs_scene_audio_window_frames(false, TICK + 999999999ULL, TICK, (size_t)1 << 62), false, 0,
		      "remainder wraps");
#endif
}

static void mixes_every_combination(void **state)
{
	(void)state;

	for (int bits = 0; bits < 8; bits++) {
		const bool apply_buf = bits & 1;
		const bool visible = bits & 2;
		const bool hiding = bits & 4;
		/* only a hidden item with no volume buffer and no hide
		 * transition is left out */
		expect_bool(obs_scene_audio_window_mixes(apply_buf, visible, hiding), apply_buf || visible || hiding,
			    "mixes");
	}
}

static void adds_unless_duplicated(void **state)
{
	(void)state;

	expect_bool(obs_scene_audio_window_adds(false), true, "not duplicated");
	expect_bool(obs_scene_audio_window_adds(true), false, "duplicated");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(pick_every_combination),
		cmocka_unit_test(child_values),
		cmocka_unit_test(counts_every_combination),
		cmocka_unit_test(earliest_over_item_sets),
		cmocka_unit_test(earliest_step),
		cmocka_unit_test(silent),
		cmocka_unit_test(window_edges_48k),
		cmocka_unit_test(window_edges_44k1),
		cmocka_unit_test(window_edges_other_rates),
		cmocka_unit_test(window_skips),
		cmocka_unit_test(window_before_the_minimum),
		cmocka_unit_test(window_u64_edges),
		cmocka_unit_test(mixes_every_combination),
		cmocka_unit_test(adds_unless_duplicated),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
