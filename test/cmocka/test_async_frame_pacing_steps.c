#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <inttypes.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-async-pacing.h"

/*
 * Characterization of the async video frame pacing of get_closest_frame()
 * and ready_async_frame() in libobs/obs-source.c, moved to
 * libobs/obs-source-async-pacing.c before its Rust port:
 * - the first-frame rule: with last_frame_ts 0 the oldest frame is shown,
 *   buffered or not, and seeds last_frame_ts;
 * - unbuffered: the newest frame, the older ones dropped;
 * - buffered: the timing jump when the oldest frame is more than MAX_TS_VAR
 *   from the source clock (frame_out_of_bounds()), then the catch-up walk
 *   with its 2 ms duplicate guard (only once a frame is passed, upstream
 *   62500b512) and its MAX_TS_VAR re-anchor;
 * - the seed of last_frame_ts when a frame is shown and it is 0.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The function is hidden in libobs, so this test compiles the C source into
 * itself and calls only it. A model queue stands in for async_frames: it
 * applies each result as get_closest_frame() does (drop_front frames off the
 * front, then the front frame taken) and stores last_sys_timestamp as
 * async_tick() does. test_media_restart_stale_frame,
 * test_media_restart_first_frame, test_async_delay_audio and the v4l2 tests
 * cover the caller through the real libobs.
 *
 * Frame timestamps come from capture devices and media files, so any u64 can
 * arrive; the C relies on uint64_t wrapping.
 */

#define MS 1000000ULL
#define SEC 1000000000ULL
#define MAX_TS_VAR OBS_SOURCE_ASYNC_PACING_MAX_TS_VAR
#define FRAME_TOL OBS_SOURCE_ASYNC_PACING_FRAME_TOLERANCE
/* a source clock well away from 0 and UINT64_MAX */
#define L (100 * SEC)
/* the canvas clock (obs->video.video_time), on another base */
#define VT (5000 * SEC)
#define QUEUE_MAX 64

static void check_u64(uint64_t actual, uint64_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %" PRIu64 ", expected %" PRIu64 "\n", line, what, actual, expected);
		fail();
	}
}

/* One call: the queue's timestamps in, the result checked field by field. */
static void check_pace(const uint64_t *ts, size_t count, uint64_t sys_time, uint64_t last_frame_ts,
		       uint64_t last_sys_timestamp, bool unbuffered, size_t drop_front, bool take_first,
		       uint64_t new_last_frame_ts, int line)
{
	struct obs_source_async_pacing_result r =
		obs_source_async_pacing(ts, count, sys_time, last_frame_ts, last_sys_timestamp, unbuffered);
	check_u64(r.drop_front, drop_front, "drop_front", line);
	check_u64(r.take_first, take_first, "take_first", line);
	check_u64(r.last_frame_ts, new_last_frame_ts, "last_frame_ts", line);
	/* the contract obs-source.c relies on */
	if (count)
		check_u64(r.drop_front < count, true, "drop_front < count", line);
	if (!r.take_first)
		check_u64(r.drop_front, 0, "drop_front without take_first", line);
}

#define PACE(ts, count, sys_time, lft, lst, unbuffered, drop, take, new_lft) \
	check_pace((ts), (count), (sys_time), (lft), (lst), (unbuffered), (drop), (take), (new_lft), __LINE__)

/* async_frames, last_frame_ts, last_sys_timestamp and async_unbuffered of a
 * source, and what async_tick() did with them. */
struct model {
	uint64_t ts[QUEUE_MAX];
	size_t num;
	uint64_t last_frame_ts;
	uint64_t last_sys_timestamp;
	bool unbuffered;
	/* totals over the ticks */
	size_t shown;
	size_t dropped;
	size_t empty_ticks;
};

static void push(struct model *m, uint64_t ts)
{
	assert_true(m->num < QUEUE_MAX);
	m->ts[m->num++] = ts;
}

static void erase_front(struct model *m)
{
	memmove(m->ts, m->ts + 1, (m->num - 1) * sizeof(m->ts[0]));
	m->num--;
}

/* get_closest_frame() and the store of last_sys_timestamp in async_tick().
 * Returns whether a frame was taken, and its timestamp in *shown. */
static bool tick(struct model *m, uint64_t sys_time, uint64_t *shown)
{
	bool taken = false;

	if (m->num) {
		struct obs_source_async_pacing_result r = obs_source_async_pacing(
			m->ts, m->num, sys_time, m->last_frame_ts, m->last_sys_timestamp, m->unbuffered);
		assert_true(r.drop_front < m->num);
		for (size_t i = 0; i < r.drop_front; i++) {
			erase_front(m);
			m->dropped++;
		}
		m->last_frame_ts = r.last_frame_ts;
		if (r.take_first) {
			*shown = m->ts[0];
			erase_front(m);
			m->shown++;
			taken = true;
		}
	}
	if (!taken)
		m->empty_ticks++;
	m->last_sys_timestamp = sys_time;
	return taken;
}

#define TICK(m, sys_time, expect_taken, expect_ts)                                        \
	do {                                                                              \
		uint64_t shown_ts_ = 0;                                                   \
		bool taken_ = tick((m), (sys_time), &shown_ts_);                          \
		check_u64(taken_, (expect_taken), "taken", __LINE__);                     \
		if (expect_taken)                                                         \
			check_u64(shown_ts_, (expect_ts), "shown timestamp", __LINE__);   \
	} while (0)

/* ------------------------------------------------------------------------- */

static void empty_queue(void **state)
{
	(void)state;
	/* get_closest_frame() returns before it reads anything; the pointer may
	 * be NULL */
	PACE(NULL, 0, VT, L, VT, false, 0, false, L);
	PACE(NULL, 0, VT, L, VT, true, 0, false, L);
	PACE(NULL, 0, VT, 0, 0, false, 0, false, 0);
}

static void first_frame_rule(void **state)
{
	(void)state;
	const uint64_t ts[3] = {L, L + 33 * MS, L + 66 * MS};

	/* last_frame_ts 0: the oldest frame, whatever the buffering, the clock
	 * or the other frames, and it seeds last_frame_ts */
	PACE(ts, 3, VT, 0, VT - 16 * MS, false, 0, true, L);
	PACE(ts, 3, VT, 0, VT - 16 * MS, true, 0, true, L);
	PACE(ts, 1, 0, 0, UINT64_MAX, false, 0, true, L);

	/* characterized, not endorsed: a frame at timestamp 0 seeds 0, so the
	 * next tick takes the first-frame rule again */
	const uint64_t zero[2] = {0, 33 * MS};
	PACE(zero, 2, VT, 0, VT, false, 0, true, 0);
	PACE(zero + 1, 1, VT, 0, VT, false, 0, true, 33 * MS);
	/* unbuffered: the oldest, not the newest */
	PACE(zero, 2, VT, 0, VT, true, 0, true, 0);
}

static void unbuffered_newest(void **state)
{
	(void)state;
	uint64_t ts[30];
	for (size_t i = 0; i < 30; i++)
		ts[i] = L + i * 33 * MS;

	/* 1, 2 and 30 frames: the newest, the others dropped; last_frame_ts
	 * becomes its timestamp; sys_time and last_sys_timestamp are not read */
	PACE(ts, 1, VT, L - SEC, VT, true, 0, true, L);
	PACE(ts, 2, VT, L - SEC, VT, true, 1, true, L + 33 * MS);
	PACE(ts, 30, VT, L - SEC, VT, true, 29, true, L + 29 * 33 * MS);
	PACE(ts, 30, 0, 1, UINT64_MAX, true, 29, true, L + 29 * 33 * MS);

	/* no jump rule and no walk: a newest frame far in the future or the past
	 * is shown at once */
	const uint64_t far[2] = {L, L + 100 * SEC};
	PACE(far, 2, VT, L, VT, true, 1, true, L + 100 * SEC);
	const uint64_t back[2] = {L, 5 * MS};
	PACE(back, 2, VT, L, VT, true, 1, true, 5 * MS);

	/* a newest timestamp of 0 seeds from the shown frame: 0 again */
	const uint64_t ends_zero[2] = {L, 0};
	PACE(ends_zero, 2, VT, L, VT, true, 1, true, 0);
}

static void unbuffered_more_than_max_frames(void **state)
{
	(void)state;
	/* cache_video() checks MAX_ASYNC_FRAMES (30) before it unlocks, so
	 * producers on several threads can queue a few more: nothing assumes 30 */
	uint64_t ts[40];
	for (size_t i = 0; i < 40; i++)
		ts[i] = L + i * 10 * MS;
	PACE(ts, 31, VT, L, VT, true, 30, true, L + 300 * MS);
	PACE(ts, 40, VT, L, VT, true, 39, true, L + 390 * MS);
	/* buffered, all in the past: the walk goes to the last one */
	PACE(ts, 40, VT + SEC, L, VT, false, 39, true, L + SEC);
}

static void timing_jump_edges(void **state)
{
	(void)state;
	const uint64_t lst = VT - 16 * MS;

	/* ahead: MAX_TS_VAR + 1 is a jump (shown at once, last_frame_ts becomes
	 * its timestamp); MAX_TS_VAR and MAX_TS_VAR - 1 are not, and the source
	 * clock (L + 16 ms) has not reached the frame: nothing new */
	uint64_t ts[1];
	ts[0] = L + MAX_TS_VAR + 1;
	PACE(ts, 1, VT, L, lst, false, 0, true, L + MAX_TS_VAR + 1);
	ts[0] = L + MAX_TS_VAR;
	PACE(ts, 1, VT, L, lst, false, 0, false, L + 16 * MS);
	ts[0] = L + MAX_TS_VAR - 1;
	PACE(ts, 1, VT, L, lst, false, 0, false, L + 16 * MS);

	/* behind: the same. Within MAX_TS_VAR the frame is past and the last
	 * one queued: shown, and the source clock keeps its advance */
	ts[0] = L - MAX_TS_VAR - 1;
	PACE(ts, 1, VT, L, lst, false, 0, true, L - MAX_TS_VAR - 1);
	ts[0] = L - MAX_TS_VAR;
	PACE(ts, 1, VT, L, lst, false, 0, true, L + 16 * MS);
	ts[0] = L - MAX_TS_VAR + 1;
	PACE(ts, 1, VT, L, lst, false, 0, true, L + 16 * MS);

	/* the jump looks only at the oldest frame and takes it, whatever is
	 * behind it */
	const uint64_t jump3[3] = {L + 3 * SEC, L + 3 * SEC + 33 * MS, L + 10 * SEC};
	PACE(jump3, 3, VT, L, lst, false, 0, true, L + 3 * SEC);

	/* the distance wraps nowhere: 0 and UINT64_MAX are 2^64 - 1 apart. A
	 * jump to timestamp 0 stores 0, and the seed from the shown frame keeps
	 * it 0 */
	ts[0] = UINT64_MAX;
	PACE(ts, 1, VT, 1, lst, false, 0, true, UINT64_MAX);
	ts[0] = 0;
	PACE(ts, 1, VT, UINT64_MAX, lst, false, 0, true, 0);
}

static void duplicate_guard_edges(void **state)
{
	(void)state;
	/* two frames behind the source clock L (no canvas time since the last
	 * tick): the first is passed, then the second is due d ago. Within the
	 * 2 ms guard (d < 2000000) the first is shown; at it and past it the
	 * first is dropped and the second shown */
	uint64_t ts[2] = {L - 50 * MS, 0};

	ts[1] = L - 1999999;
	PACE(ts, 2, VT, L, VT, false, 0, true, L);
	ts[1] = L - 2000000;
	PACE(ts, 2, VT, L, VT, false, 1, true, L);
	ts[1] = L - 2000001;
	PACE(ts, 2, VT, L, VT, false, 1, true, L);

	/* the guard is not applied before a frame is passed (62500b512): the
	 * oldest frame 1 ns due is shown */
	const uint64_t just_due[2] = {L - 1, L + 33 * MS};
	PACE(just_due, 2, VT, L, VT, false, 0, true, L);
	/* not due (the clock must be strictly past it): nothing new */
	const uint64_t at_clock[2] = {L, L + 33 * MS};
	PACE(at_clock, 2, VT, L, VT, false, 0, false, L);

	/* the guard of a later step: four frames, the walk drops two */
	const uint64_t four[4] = {L - 90 * MS, L - 60 * MS, L - 30 * MS, L - 1 * MS};
	PACE(four, 4, VT, L, VT, false, 2, true, L);
}

static void reanchor_in_walk(void **state)
{
	(void)state;
	/* frame_offset is the first frame's distance from the source clock
	 * (A - L, -10 ms). When the next frame is more than MAX_TS_VAR after the
	 * one passed, the clock moves to it less that offset: L + MAX_TS_VAR + 1,
	 * so the next frame is 10 ms past and the first is dropped */
	const uint64_t a = L - 10 * MS;
	uint64_t ts[3] = {a, a + MAX_TS_VAR + 1, a + MAX_TS_VAR + 1 + 33 * MS};
	PACE(ts, 2, VT, L, VT, false, 1, true, L + MAX_TS_VAR + 1);
	PACE(ts, 3, VT, L, VT, false, 1, true, L + MAX_TS_VAR + 1);

	/* MAX_TS_VAR after: no re-anchor, the next frame is not due, the first
	 * is shown */
	ts[1] = a + MAX_TS_VAR;
	ts[2] = a + MAX_TS_VAR + 33 * MS;
	PACE(ts, 3, VT, L, VT, false, 0, true, L);

	/* backwards by any amount wraps past MAX_TS_VAR and re-anchors: 1 s
	 * back moves the clock 1 s back, the next frame is 10 ms past */
	const uint64_t back[2] = {a, a - SEC};
	PACE(back, 2, VT, L, VT, false, 1, true, L - SEC);

	/* re-anchoring twice in one walk, with the canvas 20 ms on.
	 * Characterized, not endorsed: the first frame_offset is taken before
	 * the clock advances by sys_offset, so the re-anchor drops this tick's
	 * 20 ms: the clock lands on L + 6 s, not L + 6 s + 20 ms */
	const uint64_t twice[4] = {a, a + 3 * SEC, a + 6 * SEC, a + 6 * SEC + 50 * MS};
	PACE(twice, 4, VT + 20 * MS, L, VT, false, 2, true, L + 6 * SEC);

	/* characterized, not endorsed: a re-anchor that lands on 0 leaves the
	 * next frame not due, and the first is shown and seeds last_frame_ts */
	const uint64_t to_zero[2] = {a, UINT64_MAX - 10 * MS + 1};
	PACE(to_zero, 2, VT, L, VT, false, 0, true, a);
}

static void timestamps_backwards(void **state)
{
	(void)state;
	/* characterized, not endorsed: a frame 1 ns before the one passed wraps
	 * to a jump of 2^64 - 1 and re-anchors the clock 1 ns back, keeping the
	 * passed frame's 10 ms lead */
	const uint64_t a = L - 10 * MS;
	const uint64_t one_back[3] = {a, a - 1, a + 33 * MS};
	PACE(one_back, 3, VT, L, VT, false, 1, true, L - 1);

	/* a queue that goes back by more than the clock: each step back
	 * re-anchors, the walk drops to the last frame */
	const uint64_t descending[4] = {L - 10 * MS, L - 20 * MS, L - 30 * MS, L - 40 * MS};
	PACE(descending, 4, VT, L, VT, false, 3, true, L - 30 * MS);

	/* the oldest frame behind the clock within MAX_TS_VAR is just past */
	const uint64_t behind[1] = {L - SEC};
	PACE(behind, 1, VT, L, VT, false, 0, true, L);
}

static void sys_time_backwards(void **state)
{
	(void)state;
	/* characterized, not endorsed: sys_time before last_sys_timestamp wraps
	 * sys_offset, which moves the source clock back by the difference: a
	 * frame that was due is not shown */
	const uint64_t ts[1] = {L - 50 * MS};
	PACE(ts, 1, VT - 100 * MS, L, VT, false, 0, false, L - 100 * MS);
	/* and it is due again once the clock passes it */
	PACE(ts, 1, VT - 40 * MS, L, VT, false, 0, true, L - 40 * MS);

	/* a clock below the step wraps to near UINT64_MAX: every frame is past,
	 * the walk drops to the last */
	const uint64_t small[3] = {40 * MS, 73 * MS, 106 * MS};
	PACE(small, 3, VT - 100 * MS, 50 * MS, VT, false, 2, true, (uint64_t)0 - 50 * MS);

	/* and the tick after, the next frame is out of bounds: a jump, so the
	 * source recovers */
	struct model m = {.last_frame_ts = 50 * MS, .last_sys_timestamp = VT};
	push(&m, 40 * MS);
	push(&m, 73 * MS);
	push(&m, 106 * MS);
	TICK(&m, VT - 100 * MS, true, 106 * MS);
	check_u64(m.last_frame_ts, (uint64_t)0 - 50 * MS, "last_frame_ts", __LINE__);
	push(&m, 139 * MS);
	TICK(&m, VT - 84 * MS, true, 139 * MS);
	check_u64(m.last_frame_ts, 139 * MS, "last_frame_ts", __LINE__);
	check_u64(m.dropped, 2, "dropped", __LINE__);

	/* sys_offset wraps the clock to exactly 0: nothing is due, and last_frame_ts
	 * 0 means the next tick takes the first-frame rule */
	PACE(ts, 1, VT - L, L, VT, false, 0, false, 0);
	PACE(ts, 1, VT, 0, VT - L, false, 0, true, L - 50 * MS);
}

static void all_frames_in_the_past(void **state)
{
	(void)state;
	/* 30 frames (MAX_ASYNC_FRAMES), all behind the clock: the walk stops at
	 * one frame left, which is shown */
	uint64_t ts[30];
	for (size_t i = 0; i < 30; i++)
		ts[i] = L - SEC + i * 33 * MS;
	PACE(ts, 30, VT, L, VT, false, 29, true, L);

	/* the last 1 ms due: the guard keeps the one before */
	ts[29] = L - 1 * MS;
	PACE(ts, 30, VT, L, VT, false, 28, true, L);

	/* the canvas moved 16 ms since the last tick: the clock is L + 16 ms */
	ts[29] = L + 15 * MS;
	PACE(ts, 30, VT + 16 * MS, L, VT, false, 28, true, L + 16 * MS);
}

/* A buffered source of frames at num/den seconds a frame (source clock base
 * src), into a canvas ticking every tick_num/tick_den seconds (canvas base
 * VT), each frame queued one source interval before the canvas reaches it.
 * Exact integer clocks. */
static void run_sequence(struct model *m, uint64_t src, uint64_t num, uint64_t den, uint64_t tick_num,
			 uint64_t tick_den, size_t ticks)
{
	size_t queued = 0;
	for (size_t k = 0; k < ticks; k++) {
		const uint64_t sys_time = VT + k * tick_num * SEC / tick_den;
		/* queue every frame whose time is at most one interval after
		 * this tick */
		while (src + queued * num * SEC / den <= src + (sys_time - VT) + num * SEC / den) {
			push(m, src + queued * num * SEC / den);
			queued++;
		}
		uint64_t shown;
		(void)tick(m, sys_time, &shown);
	}
}

static void seq_buffered_rates(void **state)
{
	(void)state;
	/* 30 fps into 60: each frame for two ticks */
	struct model m = {.last_sys_timestamp = VT};
	run_sequence(&m, L, 1, 30, 1, 60, 120);
	check_u64(m.shown, 60, "30 into 60 shown", __LINE__);
	check_u64(m.dropped, 0, "30 into 60 dropped", __LINE__);
	check_u64(m.empty_ticks, 60, "30 into 60 repeats", __LINE__);

	/* 60 into 30: every other frame dropped, after the first tick, which
	 * takes the first-frame rule (the oldest, nothing dropped) */
	struct model m2 = {.last_sys_timestamp = VT};
	run_sequence(&m2, L, 1, 60, 1, 30, 60);
	check_u64(m2.shown, 60, "60 into 30 shown", __LINE__);
	check_u64(m2.dropped, 58, "60 into 30 dropped", __LINE__);
	check_u64(m2.empty_ticks, 0, "60 into 30 repeats", __LINE__);

	/* the same rate: one each tick, a tick late. The clock must be strictly
	 * past a frame, and the second frame is exactly at the second tick, so
	 * that tick shows nothing new and every frame after comes a tick late */
	struct model m3 = {.last_sys_timestamp = VT};
	run_sequence(&m3, L, 1, 30, 1, 30, 60);
	check_u64(m3.shown, 59, "30 into 30 shown", __LINE__);
	check_u64(m3.dropped, 0, "30 into 30 dropped", __LINE__);
	check_u64(m3.empty_ticks, 1, "30 into 30 repeats", __LINE__);
}

static void seq_unbuffered_rates(void **state)
{
	(void)state;
	/* 60 into 30 unbuffered: the newest each tick, after the first-frame
	 * rule */
	struct model m = {.last_sys_timestamp = VT, .unbuffered = true};
	run_sequence(&m, L, 1, 60, 1, 30, 60);
	check_u64(m.shown, 60, "unbuffered 60 into 30 shown", __LINE__);
	check_u64(m.dropped, 59, "unbuffered 60 into 30 dropped", __LINE__);
	/* the newest frame queued by the last tick (59 / 30 s, plus one source
	 * interval, truncated to ns) */
	check_u64(m.last_frame_ts, L + 118 * SEC / 60, "unbuffered last_frame_ts", __LINE__);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(empty_queue),        cmocka_unit_test(first_frame_rule),
		cmocka_unit_test(unbuffered_newest),  cmocka_unit_test(unbuffered_more_than_max_frames),
		cmocka_unit_test(timing_jump_edges),  cmocka_unit_test(duplicate_guard_edges),
		cmocka_unit_test(reanchor_in_walk),   cmocka_unit_test(timestamps_backwards),
		cmocka_unit_test(sys_time_backwards), cmocka_unit_test(all_frames_in_the_past),
		cmocka_unit_test(seq_buffered_rates), cmocka_unit_test(seq_unbuffered_rates),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
