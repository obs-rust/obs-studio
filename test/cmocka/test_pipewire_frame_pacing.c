#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <util/c99defs.h>

#include "frame-pacing.h"

/* obsproject/obs-studio#13292: recording a 60 Hz screen at 60 fps draws some
 * frames twice and then skips one. The screen capture source is synchronous:
 * each OBS video tick draws whichever compositor frame arrived last. Both run
 * at 60 Hz on independent clocks, so their phase is arbitrary; whenever the
 * compositor's frames land near OBS's tick, a little delivery jitter makes one
 * tick see two new frames and the next none.
 *
 * These tests replay a compositor at a fixed rate with deterministic jitter
 * against OBS's video ticks, and count what the recording would contain. */

#define OBS_INTERVAL_NS 16666667ULL /* 60 fps output */
#define WARMUP_TICKS 10
#define SIM_TICKS 600 /* 10 seconds */
#define MAX_SIM_FRAMES (SIM_TICKS * 3)

struct sim_result {
	int dups;    /* ticks that redraw the previous frame */
	int skips;   /* frames never drawn */
	int max_lag; /* frames between the newest arrival and the drawn one */
	int released[MAX_SIM_FRAMES];
	int pushed;
};

static uint32_t lcg(uint32_t *seed)
{
	*seed = *seed * 1664525u + 1013904223u;
	return *seed >> 8;
}

/* Jitter in [-jitter_ns, +jitter_ns]. */
static int64_t next_jitter(uint32_t *seed, int64_t jitter_ns)
{
	if (!jitter_ns)
		return 0;
	return (int64_t)(lcg(seed) % (uint32_t)(2 * jitter_ns + 1)) - jitter_ns;
}

static void note_released(struct sim_result *r, uint64_t frame)
{
	if (frame == OBS_PW_FRAME_NONE)
		return;
	assert_true(frame < (uint64_t)r->pushed);
	r->released[frame]++;
}

/* The compositor sends frame k at k * source_interval + phase + jitter (the
 * first one interval in, so jitter never makes a time negative); OBS ticks at
 * n * OBS_INTERVAL_NS and draws the source right after. */
static void simulate(uint64_t source_interval_ns, int64_t phase_ns, int64_t jitter_ns, struct sim_result *r)
{
	struct obs_pw_frame_pacer pacer;
	uint32_t seed = 13292;
	uint64_t prev = OBS_PW_FRAME_NONE;
	uint64_t next_arrival;

	*r = (struct sim_result){0};
	obs_pw_frame_pacer_init(&pacer);

	next_arrival = source_interval_ns + phase_ns + next_jitter(&seed, jitter_ns);

	for (int n = 1; n <= SIM_TICKS; n++) {
		uint64_t tick_time = n * OBS_INTERVAL_NS;
		uint64_t released;
		uint64_t drawn;

		while (next_arrival <= tick_time && r->pushed < MAX_SIM_FRAMES) {
			note_released(r, obs_pw_frame_pacer_push(&pacer, r->pushed++));
			next_arrival = (r->pushed + 1) * source_interval_ns + phase_ns + next_jitter(&seed, jitter_ns);
		}

		drawn = obs_pw_frame_pacer_tick(&pacer, &released);
		note_released(r, released);
		if (drawn == OBS_PW_FRAME_NONE)
			continue;

		/* A frame the pacer released must not be drawn any more. */
		assert_int_equal(r->released[drawn], 0);

		if (n > WARMUP_TICKS && prev != OBS_PW_FRAME_NONE) {
			if (drawn == prev)
				r->dups++;
			else if (drawn > prev + 1)
				r->skips += (int)(drawn - prev - 1);
			assert_true(drawn >= prev);
		}
		if (n > WARMUP_TICKS && r->pushed - 1 - (int)drawn > r->max_lag)
			r->max_lag = r->pushed - 1 - (int)drawn;
		prev = drawn;
	}

	uint64_t flushed[OBS_PW_FRAME_PACER_MAX_FRAMES + 1];
	size_t count = obs_pw_frame_pacer_flush(&pacer, flushed);

	assert_true(count <= OBS_PW_FRAME_PACER_MAX_FRAMES + 1);
	for (size_t i = 0; i < count; i++)
		note_released(r, flushed[i]);
}

static void nothing_is_drawn_before_the_first_frame(void **state)
{
	UNUSED_PARAMETER(state);
	struct obs_pw_frame_pacer pacer;
	uint64_t released;

	obs_pw_frame_pacer_init(&pacer);
	assert_true(obs_pw_frame_pacer_tick(&pacer, &released) == OBS_PW_FRAME_NONE);
	assert_true(released == OBS_PW_FRAME_NONE);
}

/* Control: steady frames halfway between ticks are each drawn once. */
static void steady_frames_between_ticks_are_each_drawn_once(void **state)
{
	UNUSED_PARAMETER(state);
	struct sim_result r;

	simulate(OBS_INTERVAL_NS, OBS_INTERVAL_NS / 2, 0, &r);
	assert_int_equal(r.dups, 0);
	assert_int_equal(r.skips, 0);
}

/* The bug: the same 60 Hz compositor with +-2 ms of delivery jitter, at every
 * phase against OBS's clock. No frame may be skipped, and a frame may be
 * drawn twice at most once per run: a one-off step to a steady delay, not
 * the recurring double-then-skip of the report. */
static void jittery_frames_are_each_drawn_once_at_any_phase(void **state)
{
	UNUSED_PARAMETER(state);
	const int64_t jitter_ns = 2000000;
	const int64_t step_ns = 500000;
	int bad_phases = 0, phases = 0, dups = 0, skips = 0;
	struct sim_result r;

	for (int64_t phase = 0; phase < (int64_t)OBS_INTERVAL_NS; phase += step_ns) {
		simulate(OBS_INTERVAL_NS, phase, jitter_ns, &r);
		phases++;
		dups += r.dups;
		skips += r.skips;
		if (r.dups > 1 || r.skips)
			bad_phases++;
	}

	print_message(
		"60 Hz source, +-2 ms jitter, %d phases x %d ticks: %d bad phases, %d duplicated and %d skipped frames\n",
		phases, SIM_TICKS - WARMUP_TICKS, bad_phases, dups, skips);
	assert_int_equal(bad_phases, 0);
}

/* The compositor's clock (the display's vblank) and OBS's drift apart: a
 * source 1% faster than the output, with the same jitter, has about 6 more
 * frames than OBS has ticks in 10 seconds. Only those may be lost, and no
 * frame may be drawn twice beyond the one-off step. */
static void drifting_source_loses_only_its_surplus_frames(void **state)
{
	UNUSED_PARAMETER(state);
	const int64_t jitter_ns = 2000000;
	int worst_dups = 0, worst_skips = 0;
	struct sim_result r;

	for (int64_t phase = 0; phase < (int64_t)OBS_INTERVAL_NS; phase += 2000000) {
		simulate(OBS_INTERVAL_NS * 100 / 101, phase, jitter_ns, &r);
		if (r.dups > worst_dups)
			worst_dups = r.dups;
		if (r.skips > worst_skips)
			worst_skips = r.skips;
	}

	print_message("60.6 Hz source, +-2 ms jitter: at worst %d duplicated and %d skipped frames\n", worst_dups,
		      worst_skips);
	assert_true(worst_dups <= 1);
	assert_true(worst_skips <= 7);
}

/* A source faster than the output must drop frames rather than fall further
 * and further behind. */
static void faster_source_keeps_latency_bounded(void **state)
{
	UNUSED_PARAMETER(state);
	struct sim_result r;

	simulate(OBS_INTERVAL_NS / 2, 1000000, 1000000, &r);
	assert_true(r.max_lag < OBS_PW_FRAME_PACER_MAX_FRAMES);
	assert_int_equal(r.dups, 0);
}

/* A slower source is redrawn, never reordered. */
static void slower_source_repeats_frames(void **state)
{
	UNUSED_PARAMETER(state);
	struct sim_result r;

	simulate(OBS_INTERVAL_NS * 2, 1000000, 1000000, &r);
	assert_true(r.dups > 0);
	assert_int_equal(r.skips, 0);
}

/* Every buffer goes back to PipeWire exactly once, or the stream runs dry. */
static void every_frame_is_released_once(void **state)
{
	UNUSED_PARAMETER(state);
	static const uint64_t intervals[] = {OBS_INTERVAL_NS / 2, OBS_INTERVAL_NS, OBS_INTERVAL_NS * 2};
	struct sim_result r;

	for (size_t i = 0; i < sizeof(intervals) / sizeof(intervals[0]); i++) {
		simulate(intervals[i], 300000, 2000000, &r);
		assert_true(r.pushed > 0);
		for (int f = 0; f < r.pushed; f++)
			assert_int_equal(r.released[f], 1);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(nothing_is_drawn_before_the_first_frame),
		cmocka_unit_test(steady_frames_between_ticks_are_each_drawn_once),
		cmocka_unit_test(jittery_frames_are_each_drawn_once_at_any_phase),
		cmocka_unit_test(drifting_source_loses_only_its_surplus_frames),
		cmocka_unit_test(faster_source_keeps_latency_bounded),
		cmocka_unit_test(slower_source_repeats_frames),
		cmocka_unit_test(every_frame_is_released_once),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
