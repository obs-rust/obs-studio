#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <inttypes.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-deinterlace-pacing.h"

/*
 * Characterization of the 2x deinterlace frame pacing of
 * deinterlace_get_closest_frames() and ready_deinterlace_frames() in
 * libobs/obs-source-deinterlace.c, moved to
 * libobs/obs-source-deinterlace-pacing.c before its Rust port:
 * - obs_source_deinterlace_pacing_hold(): an unbuffered source keeps its
 *   frame while the frame's end is ahead of the clock by less than
 *   TS_JUMP_THRESHOLD;
 * - obs_source_deinterlace_pacing_wait_for_next() and
 *   obs_source_deinterlace_pacing_waiting_frame_ts(): the "don't skip ahead"
 *   guard of an unbuffered source with two frames queued;
 * - obs_source_deinterlace_pacing_half_duration(): half a frame's duration,
 *   truncated to 32 bits;
 * - obs_source_deinterlace_pacing_buffered_offset(): the offset of a
 *   buffered source, from the source clock that picks frames (#395);
 * - obs_source_deinterlace_pacing_reanchor(): the offset of an unbuffered
 *   source, kept unless the frame comes early or misses its tick and its
 *   first field (#395).
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. The seq_* cases run an unbuffered source
 * through a model of deinterlace_get_closest_frames(): the hold, the half
 * duration and the re-anchor, at the field and canvas rates of
 * test_deinterlace_2x_field_order, which covers the caller through the real
 * libobs (upstream obsproject/obs-studio#6061 and #3402).
 *
 * Frame timestamps come from capture devices and media files, so any u64 can
 * arrive; the C relies on uint64_t wrapping.
 */

#define MS 1000000ULL
#define TWOX OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE
#define FRAME_TOL OBS_SOURCE_DEINTERLACE_PACING_FRAME_TOLERANCE
#define JUMP OBS_SOURCE_DEINTERLACE_PACING_TS_JUMP_THRESHOLD
/* the canvas clock (obs->video.video_time) */
#define VT 1000000000000ULL
/* a frame timestamp on another clock, behind VT */
#define TS 5000000000ULL
/* 25 interlaced frames per second (50i): 40 ms frames, 20 ms fields */
#define FRAME_50I (40 * MS)
#define FIELD_50I (20 * MS)
/* 60 and 30 fps canvas intervals, as obs->video.video_frame_interval_ns */
#define TICK_60 16666666ULL
#define TICK_30 33333333ULL

static void check_u64(uint64_t actual, uint64_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %" PRIu64 ", expected %" PRIu64 "\n", line, what, actual, expected);
		fail();
	}
}

static void check_u32(uint32_t actual, uint32_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %" PRIu32 ", expected %" PRIu32 "\n", line, what, actual, expected);
		fail();
	}
}

static void check_bool(bool actual, bool expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %d, expected %d\n", line, what, actual, expected);
		fail();
	}
}

#define HOLD(frame_ts, offset, half, sys_time, expected)                                                   \
	check_bool(obs_source_deinterlace_pacing_hold((frame_ts), (offset), (half), (sys_time)), (expected), \
		   "hold", __LINE__)
#define WAIT(ts, next_ts, offset, sys_time, expected)                                                       \
	check_bool(obs_source_deinterlace_pacing_wait_for_next((ts), (next_ts), (offset), (sys_time)), (expected), \
		   "wait_for_next", __LINE__)
#define WAITING_TS(ts, next_ts, expected)                                                                    \
	check_u64(obs_source_deinterlace_pacing_waiting_frame_ts((ts), (next_ts)), (expected), "waiting_frame_ts", \
		  __LINE__)
#define HALF(ts, prev_ts, expected) \
	check_u32(obs_source_deinterlace_pacing_half_duration((ts), (prev_ts)), (expected), "half_duration", __LINE__)
#define BUFFERED(video_time, last_frame_ts, expected)                                                    \
	check_u64(obs_source_deinterlace_pacing_buffered_offset((video_time), (last_frame_ts)), (expected), \
		  "buffered_offset", __LINE__)
#define REANCHOR(offset, video_time, frame_ts, half, interval, expected)                                       \
	check_u64(obs_source_deinterlace_pacing_reanchor((offset), (video_time), (frame_ts), (half), (interval)), \
		  (expected), "reanchor", __LINE__)

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_pacing_hold                                        */

/* The frame ends at frame_ts + offset + both fields, less TWOX_TOLERANCE; the
 * source keeps it while the clock is before that, and moves on at it. */
static void hold_until_frame_end(void **state)
{
	(void)state;
	const uint64_t offset = VT - TS;
	const uint64_t end = TS + offset + 2 * FIELD_50I - TWOX; /* VT + 39 ms */

	HOLD(TS, offset, FIELD_50I, VT, true);
	HOLD(TS, offset, FIELD_50I, VT + 20 * MS, true);
	HOLD(TS, offset, FIELD_50I, end - 1, true);
	HOLD(TS, offset, FIELD_50I, end, false);
	HOLD(TS, offset, FIELD_50I, end + 1, false);
	HOLD(TS, offset, FIELD_50I, end + FRAME_50I, false);
	/* the clock behind the frame start counts too: only the end matters */
	HOLD(TS, offset, FIELD_50I, VT - 10 * MS, true);
}

/* A frame end TS_JUMP_THRESHOLD or more ahead means the clock jumped (a new
 * clock, a timestamp reset): the source moves on. Strict. */
static void hold_at_jump_threshold(void **state)
{
	(void)state;
	const uint64_t offset = VT - TS;
	const uint64_t end = TS + offset + 2 * FIELD_50I - TWOX;

	HOLD(TS, offset, FIELD_50I, end - (JUMP - 1), true);
	HOLD(TS, offset, FIELD_50I, end - JUMP, false);
	HOLD(TS, offset, FIELD_50I, end - (JUMP + 1), false);
	HOLD(TS, offset, FIELD_50I, 0, false);
}

/* The half duration is widened before it is doubled: UINT32_MAX gives an end
 * 8.6 s ahead, not a 32-bit wrap. A frame that long is past the threshold. */
static void hold_half_duration_widened(void **state)
{
	(void)state;
	const uint64_t offset = VT - TS;
	const uint64_t end = VT + 2 * (uint64_t)UINT32_MAX - TWOX;

	HOLD(TS, offset, UINT32_MAX, end - 1, true);
	HOLD(TS, offset, UINT32_MAX, end, false);
	HOLD(TS, offset, UINT32_MAX, end - JUMP, false);
	/* a half duration of 0: the end is TWOX_TOLERANCE before the start */
	HOLD(TS, offset, 0, VT - TWOX - 1, true);
	HOLD(TS, offset, 0, VT - TWOX, false);
}

/* Characterized, not endorsed: the end wraps. Past UINT64_MAX it lands near 0,
 * so no clock is before it; a sum below TWOX_TOLERANCE lands near UINT64_MAX,
 * so every clock is before it by more than the threshold. */
static void hold_end_wraps(void **state)
{
	(void)state;
	/* frame_ts + offset = 2^64 + 10 ms: the end is 49 ms */
	HOLD(UINT64_MAX, 10 * MS + 1, FIELD_50I, 0, true);
	HOLD(UINT64_MAX, 10 * MS + 1, FIELD_50I, 48 * MS, true);
	HOLD(UINT64_MAX, 10 * MS + 1, FIELD_50I, 49 * MS, false);
	HOLD(UINT64_MAX, 10 * MS + 1, FIELD_50I, VT, false);
	/* the end is UINT64_MAX - TWOX_TOLERANCE + 1 */
	HOLD(0, 0, 0, 0, false);
	HOLD(0, 0, 0, UINT64_MAX - TWOX, true);
	HOLD(0, 0, 0, UINT64_MAX - TWOX + 1, false);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_pacing_wait_for_next / _waiting_frame_ts           */

/* Two frames queued: wait while the clock is before the first one's end,
 * timestamp + offset + its duration (no TWOX_TOLERANCE here). The waiting
 * frame's timestamp is set one duration back, so its half duration is half
 * of it. */
static void wait_until_first_frame_ends(void **state)
{
	(void)state;
	const uint64_t offset = VT - TS;
	const uint64_t end = VT + FRAME_50I;

	WAIT(TS, TS + FRAME_50I, offset, VT, true);
	WAIT(TS, TS + FRAME_50I, offset, end - 1, true);
	WAIT(TS, TS + FRAME_50I, offset, end, false);
	WAIT(TS, TS + FRAME_50I, offset, end + 1, false);
	/* no threshold: far behind still waits */
	WAIT(TS, TS + FRAME_50I, offset, 0, true);
	/* frames at the same time: the end is the start */
	WAIT(TS, TS, offset, VT - 1, true);
	WAIT(TS, TS, offset, VT, false);

	WAITING_TS(TS, TS + FRAME_50I, TS - FRAME_50I);
	WAITING_TS(TS, TS, TS);
	check_u32(obs_source_deinterlace_pacing_half_duration(
			  TS, obs_source_deinterlace_pacing_waiting_frame_ts(TS, TS + FRAME_50I)),
		  FIELD_50I, "half_duration", __LINE__);
}

/* Characterized, not endorsed: a next frame before the first (a timestamp
 * going backwards) wraps the duration. The end is then the next frame's
 * timestamp plus the offset (the sum is modular), and the waiting frame's
 * timestamp is as far after the first as the next is before it. */
static void wait_timestamp_backwards_wraps(void **state)
{
	(void)state;
	const uint64_t offset = VT - TS;

	WAIT(TS, TS - FRAME_50I, offset, VT - FRAME_50I - 1, true);
	WAIT(TS, TS - FRAME_50I, offset, VT - FRAME_50I, false);
	WAIT(TS, TS - FRAME_50I, offset, VT, false);
	WAITING_TS(TS, TS - FRAME_50I, TS + FRAME_50I);
	/* the end wraps past UINT64_MAX */
	WAIT(UINT64_MAX - 10 * MS, UINT64_MAX, 20 * MS, 9 * MS, true);
	WAIT(UINT64_MAX - 10 * MS, UINT64_MAX, 20 * MS, 20 * MS - 2, true);
	WAIT(UINT64_MAX - 10 * MS, UINT64_MAX, 20 * MS, 20 * MS - 1, false);
	/* a frame near 0: one duration back wraps below 0 */
	WAITING_TS(10 * MS, 50 * MS, UINT64_MAX - 30 * MS + 1);
	WAITING_TS(0, UINT64_MAX, 1);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_pacing_half_duration                               */

static void half_duration_of_a_frame(void **state)
{
	(void)state;
	HALF(TS + FRAME_50I, TS, FIELD_50I);
	/* 59.94i: 1001/30000 s frames */
	HALF(TS + 33366667, TS, 16683333);
	/* rounds down */
	HALF(TS + 1, TS, 0);
	HALF(TS + 3, TS, 1);
	HALF(TS, TS, 0);
	/* the largest that fits */
	HALF(TS + 2 * (uint64_t)UINT32_MAX + 1, TS, UINT32_MAX);
}

/* Characterized, not endorsed: half a duration of 2^32 ns (8.6 s) or more is
 * truncated to 32 bits, so a long gap gives a short half duration. */
static void half_duration_over_2_32_truncated(void **state)
{
	(void)state;
	HALF(TS + 2 * ((uint64_t)UINT32_MAX + 1), TS, 0);
	HALF(TS + 2 * ((uint64_t)UINT32_MAX + 1) + 6, TS, 3);
	/* a 10 s gap: 5e9 ns, kept as 705032704 ns */
	HALF(TS + 10000000000ULL, TS, 705032704u);
	HALF(UINT64_MAX, 0, UINT32_MAX);
}

/* Characterized, not endorsed: a frame before the one it follows (a
 * timestamp going backwards) wraps the difference, and half of it truncated
 * is 2^32 - 20 ms for a frame 40 ms back. */
static void half_duration_timestamp_backwards_wraps(void **state)
{
	(void)state;
	HALF(TS - FRAME_50I, TS, (uint32_t)(((uint64_t)0 - FRAME_50I) / 2));
	HALF(TS - FRAME_50I, TS, 4274967296u);
	HALF(TS - 1, TS, UINT32_MAX);
	HALF(0, UINT64_MAX, 0);
}

/* Characterized, not endorsed: the first frame, or the first after the queue
 * was reset (last_frame_ts 0), with no second frame queued has no previous
 * one, so its half duration is taken against deinterlace_frame_ts: 0 when
 * the source never showed a frame, so half the frame's own timestamp,
 * truncated to 32 bits; the last frame shown before, after a pause. */
static void half_duration_first_frame(void **state)
{
	(void)state;
	/* a capture timestamp on the system clock, 1000 s after boot:
	 * 5e11 ns, kept as 1.78 s */
	HALF(VT, 0, 1783793664u);
	/* a media timestamp of 5 s: 2.5 s */
	HALF(TS, 0, 2500000000u);
	/* a capture timestamp that leaves 30 ms */
	HALF(2 * (100 * ((uint64_t)UINT32_MAX + 1) + 30 * MS), 0, 30 * MS);
	/* a first frame at 0 */
	HALF(0, 0, 0);
	/* after a 10 s pause, against the last frame shown: 705 ms */
	HALF(TS + 10000000000ULL, TS, 705032704u);
}

/* What the first frame's half duration does next. An unbuffered source's
 * first pick re-anchors (the offset is 0), so the frame's end is the pick
 * plus twice the half duration, less TWOX_TOLERANCE. Characterized, not
 * endorsed. */
static void first_frame_half_duration_hold(void **state)
{
	(void)state;
	/* a capture timestamp 5 ms before the tick, 1000 s after boot: 1.78
	 * s, so the end is 3.57 s ahead, past TS_JUMP_THRESHOLD, and the next
	 * tick moves on, as if the clock had jumped */
	uint64_t ts = VT - 5 * MS;
	uint32_t half = obs_source_deinterlace_pacing_half_duration(ts, 0);
	check_u32(half, 1781293664u, "half", __LINE__);
	uint64_t offset = obs_source_deinterlace_pacing_reanchor(0, VT, ts, half, TICK_60);
	check_u64(offset, 5 * MS, "offset", __LINE__);
	HOLD(ts, offset, half, VT + TICK_60, false);

	/* 30 ms: held to 59 ms after the pick, where a 50i frame ends at
	 * 39 ms: one tick more at 60 fps */
	ts = 2 * (100 * ((uint64_t)UINT32_MAX + 1) + 30 * MS);
	half = obs_source_deinterlace_pacing_half_duration(ts, 0);
	offset = obs_source_deinterlace_pacing_reanchor(0, VT, ts, half, TICK_60);
	check_u64(offset, VT - ts, "offset", __LINE__);
	HOLD(ts, offset, half, VT + 3 * TICK_60, true);
	HOLD(ts, offset, half, VT + 59 * MS - 1, true);
	HOLD(ts, offset, half, VT + 59 * MS, false);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_pacing_buffered_offset                             */

/* The offset maps the source clock to the canvas clock, plus the 2 ms the
 * frame picker waits and TWOX_TOLERANCE (#395). */
static void buffered_offset(void **state)
{
	(void)state;
	BUFFERED(VT, TS, VT - TS + FRAME_TOL + TWOX);
	BUFFERED(VT, TS, VT - TS + 3 * MS);
	BUFFERED(VT, VT, 3 * MS);
	BUFFERED(0, 0, 3 * MS);
}

/* Characterized, not endorsed: a source clock ahead of the canvas clock
 * wraps the offset, and so does the sum at the end of the range. The second
 * field's time, frame_ts + offset + half, is modular, so it still lands where
 * the mapping puts it. */
static void buffered_offset_wraps(void **state)
{
	(void)state;
	BUFFERED(TS, VT, TS - VT + 3 * MS);
	BUFFERED(TS, TS + 3 * MS, 0);
	BUFFERED(TS, TS + 3 * MS + 1, UINT64_MAX);
	BUFFERED(UINT64_MAX, 0, 3 * MS - 1);
	BUFFERED(0, UINT64_MAX, 3 * MS + 1);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_pacing_reanchor                                    */

/* No offset yet: anchor at the tick that picked the frame. */
static void reanchor_without_offset(void **state)
{
	(void)state;
	REANCHOR(0, VT, TS, FIELD_50I, TICK_60, VT - TS);
	REANCHOR(0, VT + 5 * MS, TS, FIELD_50I, TICK_60, VT + 5 * MS - TS);
	/* even when the new one is 0 too */
	REANCHOR(0, VT, VT, FIELD_50I, TICK_60, 0);
}

/* A frame earlier than the offset (late < 0) re-anchors; on time or later by
 * less than a field or a tick keeps the offset. */
static void reanchor_early_frame(void **state)
{
	(void)state;
	const uint64_t offset = VT - TS;

	REANCHOR(offset, VT - 1, TS, FIELD_50I, TICK_60, offset - 1);
	REANCHOR(offset, VT - 30 * MS, TS, FIELD_50I, TICK_60, offset - 30 * MS);
	REANCHOR(offset, VT, TS, FIELD_50I, TICK_60, offset);
	REANCHOR(offset, VT + 1, TS, FIELD_50I, TICK_60, offset);
	REANCHOR(offset, VT + 10 * MS, TS, FIELD_50I, TICK_60, offset);
}

/* Late by the larger of a field and a tick, less TWOX_TOLERANCE, re-anchors
 * (the frame missed its tick and its first field). */
static void reanchor_late_by_field_or_tick(void **state)
{
	(void)state;
	const uint64_t offset = VT - TS;

	/* 50i into 60 fps: the field (20 ms) is longer than the tick */
	REANCHOR(offset, VT + FIELD_50I - TWOX - 1, TS, FIELD_50I, TICK_60, offset);
	REANCHOR(offset, VT + FIELD_50I - TWOX, TS, FIELD_50I, TICK_60, offset + FIELD_50I - TWOX);
	REANCHOR(offset, VT + FIELD_50I - TWOX + 1, TS, FIELD_50I, TICK_60, offset + FIELD_50I - TWOX + 1);
	/* 50i into 30 fps: the tick (33.3 ms) is longer than the field */
	REANCHOR(offset, VT + FIELD_50I - TWOX, TS, FIELD_50I, TICK_30, offset);
	REANCHOR(offset, VT + TICK_30 - TWOX - 1, TS, FIELD_50I, TICK_30, offset);
	REANCHOR(offset, VT + TICK_30 - TWOX, TS, FIELD_50I, TICK_30, offset + TICK_30 - TWOX);
	/* a field as long as the tick */
	REANCHOR(offset, VT + TICK_60 - TWOX - 1, TS, (uint32_t)TICK_60, TICK_60, offset);
	REANCHOR(offset, VT + TICK_60 - TWOX, TS, (uint32_t)TICK_60, TICK_60, offset + TICK_60 - TWOX);
	/* a field and a tick at most TWOX_TOLERANCE: any lateness re-anchors */
	REANCHOR(offset, VT, TS, 0, 0, offset);
	REANCHOR(offset, VT + 1, TS, 0, 0, offset + 1);
	REANCHOR(offset, VT + 1, TS, (uint32_t)TWOX, TWOX, offset + 1);
	/* a half duration over UINT32_MAX does not exist: the field is 32-bit,
	 * the tick is not */
	REANCHOR(offset, VT + 5000000000ULL - TWOX - 1, TS, FIELD_50I, 5000000000ULL, offset);
	REANCHOR(offset, VT + 5000000000ULL - TWOX, TS, FIELD_50I, 5000000000ULL, offset + 5000000000ULL - TWOX);
}

/* Characterized, not endorsed: the new offset and its difference from the old
 * one wrap, and the difference is taken as int64_t. A frame ahead of the
 * canvas clock gives an offset near UINT64_MAX; a new offset 2^63 or more
 * after the old one counts as early. */
static void reanchor_wraps(void **state)
{
	(void)state;
	/* a frame timestamp ahead of the canvas clock */
	REANCHOR(0, VT, VT + 10 * MS, FIELD_50I, TICK_60, UINT64_MAX - 10 * MS + 1);
	/* and then 1 ms later than that offset: kept */
	REANCHOR(UINT64_MAX - 10 * MS + 1, VT + 1 * MS, VT + 10 * MS, FIELD_50I, TICK_60, UINT64_MAX - 10 * MS + 1);
	/* the old offset near UINT64_MAX, the new one small: late wraps to a
	 * small positive value */
	REANCHOR(UINT64_MAX, VT, VT, FIELD_50I, TICK_60, UINT64_MAX);
	REANCHOR(UINT64_MAX, VT + FIELD_50I - TWOX - 1, VT, FIELD_50I, TICK_60, FIELD_50I - TWOX - 1);
	REANCHOR(UINT64_MAX, VT + FIELD_50I - TWOX - 2, VT, FIELD_50I, TICK_60, UINT64_MAX);
	/* INT64_MAX late: late, re-anchored */
	REANCHOR(1, VT, VT - (uint64_t)INT64_MAX - 1, FIELD_50I, TICK_60, (uint64_t)INT64_MAX + 1);
	/* 2^63 late: INT64_MIN, so early, re-anchored */
	REANCHOR(1, VT, VT - (uint64_t)INT64_MAX - 2, FIELD_50I, TICK_60, (uint64_t)INT64_MAX + 2);
	/* the largest late that is kept, and the smallest early */
	REANCHOR(10 * MS, VT, VT - 10 * MS - (FIELD_50I - TWOX - 1), FIELD_50I, TICK_60, 10 * MS);
	REANCHOR(2, 0, 0, FIELD_50I, TICK_60, 0);
	REANCHOR(UINT64_MAX, 0, 1, FIELD_50I, TICK_60, UINT64_MAX);
	REANCHOR(1, 0, 0, FIELD_50I, TICK_60, 0);
}

/* ------------------------------------------------------------------------ */
/* Sequences: an unbuffered source through a model of                        */
/* deinterlace_get_closest_frames()                                          */

/* n * 1e9 * den / num: the n-th frame or tick of a rate num/den. */
static uint64_t rate_time(uint64_t n, uint64_t num, uint64_t den)
{
	return n * 1000000000ULL * den / num;
}

struct seq_result {
	size_t reanchors; /* offsets changed after the first pick */
	size_t skipped;   /* frames never picked */
	size_t early;     /* picks before the frame before had ended */
};

/* The model: an unbuffered source of frame_num/frame_den frames a second on a
 * canvas of canvas_num/canvas_den ticks a second. Frame k arrives at the first
 * tick plus k frame durations. At every tick the source keeps its frame while
 * hold() says so; else it picks the newest frame that has arrived (the
 * queue drops the older ones), takes its half duration from the frame shown
 * before (one frame duration back for the first) and re-anchors. */
static struct seq_result run_unbuffered(uint64_t frame_num, uint64_t frame_den, uint64_t canvas_num,
					uint64_t canvas_den, size_t frames)
{
	struct seq_result r = {0, 0, 0};
	const uint64_t interval = 1000000000ULL * canvas_den / canvas_num;
	const uint64_t first_tick = VT + rate_time(1, canvas_num, canvas_den);
	const int64_t count = (int64_t)frames;
	uint64_t frame_ts = 0;
	uint64_t offset = 0;
	uint32_t half = 0;
	int64_t last = -1; /* the frame shown, -1 before the first */

	for (uint64_t tick = 1; last + 1 < count; tick++) {
		const uint64_t video_time = VT + rate_time(tick, canvas_num, canvas_den);
		if (offset && obs_source_deinterlace_pacing_hold(frame_ts, offset, half, video_time))
			continue;

		int64_t next = last;
		while (next + 1 < count &&
		       first_tick + rate_time((uint64_t)(next + 1), frame_num, frame_den) <= video_time)
			next++;
		if (next == last)
			continue;

		const uint64_t ts = TS + rate_time((uint64_t)next, frame_num, frame_den);
		uint64_t prev_ts = ts - rate_time(1, frame_num, frame_den);
		if (last >= 0) {
			r.skipped += (size_t)(next - last - 1);
			if (video_time < frame_ts + offset + 2 * (uint64_t)half - TWOX)
				r.early++;
			prev_ts = frame_ts;
		}
		half = obs_source_deinterlace_pacing_half_duration(ts, prev_ts);
		frame_ts = ts;
		const uint64_t before = offset;
		offset = obs_source_deinterlace_pacing_reanchor(offset, video_time, frame_ts, half, interval);
		if (last >= 0 && offset != before)
			r.reanchors++;
		last = next;
	}
	return r;
}

static void check_seq(uint64_t frame_num, uint64_t frame_den, uint64_t canvas_num, uint64_t canvas_den,
		      size_t reanchors, size_t skipped, int line)
{
	const struct seq_result r = run_unbuffered(frame_num, frame_den, canvas_num, canvas_den, 100);
	check_u64(r.reanchors, reanchors, "re-anchors", line);
	check_u64(r.skipped, skipped, "skipped frames", line);
	check_u64(r.early, 0, "early picks", line);
}

/* 25 and 50 frames a second (50i, 50p-rate 2x) into 30 and 60 fps: the tick
 * lags the frame by less than a field or a tick, so the offset is never
 * re-anchored (#395: re-anchoring at every pick held each frame for whole
 * ticks, and 50i fell behind 60 fps). Into 30 fps, 50 frames a second drop 76
 * of 100 frames: each tick shows the newest. */
static void seq_unbuffered_25_and_50(void **state)
{
	(void)state;
	check_seq(25, 1, 30, 1, 0, 0, __LINE__);
	check_seq(25, 1, 60, 1, 0, 0, __LINE__);
	check_seq(50, 1, 30, 1, 0, 76, __LINE__);
	check_seq(50, 1, 60, 1, 0, 0, __LINE__);
}

/* 29.97 and 59.94 frames a second (NTSC) into 30 and 60 fps. Characterized,
 * not endorsed: the canvas ticks a little faster than the frames come, so
 * once a tick lands within TWOX_TOLERANCE before a frame's time, the frame
 * counts as early and the offset re-anchors to the tick, 33 or 17 us back
 * each frame: the offset follows the canvas. 59.94 into 30 fps drops every
 * other frame and never re-anchors. */
static void seq_unbuffered_ntsc(void **state)
{
	(void)state;
	check_seq(30000, 1001, 30, 1, 99, 0, __LINE__);
	check_seq(30000, 1001, 60, 1, 99, 0, __LINE__);
	check_seq(60000, 1001, 30, 1, 0, 49, __LINE__);
	check_seq(60000, 1001, 60, 1, 99, 0, __LINE__);
}

/* A frame that comes a field late re-anchors once, then the offset holds. */
static void seq_unbuffered_late_frame(void **state)
{
	(void)state;
	const uint64_t offset = VT - TS;
	uint64_t o = offset;

	o = obs_source_deinterlace_pacing_reanchor(o, VT + 1 * FRAME_50I + 5 * MS, TS + FRAME_50I, FIELD_50I, TICK_60);
	check_u64(o, offset, "offset", __LINE__);
	/* picked 25 ms after its time: past a field less TWOX_TOLERANCE */
	o = obs_source_deinterlace_pacing_reanchor(o, VT + 2 * FRAME_50I + 25 * MS, TS + 2 * FRAME_50I, FIELD_50I,
						   TICK_60);
	check_u64(o, offset + 25 * MS, "offset", __LINE__);
	/* the next frame 10 ms after the new anchor: kept */
	o = obs_source_deinterlace_pacing_reanchor(o, VT + 3 * FRAME_50I + 35 * MS, TS + 3 * FRAME_50I, FIELD_50I,
						   TICK_60);
	check_u64(o, offset + 25 * MS, "offset", __LINE__);
	/* one back on the old anchor: early, re-anchored back */
	o = obs_source_deinterlace_pacing_reanchor(o, VT + 4 * FRAME_50I, TS + 4 * FRAME_50I, FIELD_50I, TICK_60);
	check_u64(o, offset, "offset", __LINE__);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(hold_until_frame_end),
		cmocka_unit_test(hold_at_jump_threshold),
		cmocka_unit_test(hold_half_duration_widened),
		cmocka_unit_test(hold_end_wraps),
		cmocka_unit_test(wait_until_first_frame_ends),
		cmocka_unit_test(wait_timestamp_backwards_wraps),
		cmocka_unit_test(half_duration_of_a_frame),
		cmocka_unit_test(half_duration_over_2_32_truncated),
		cmocka_unit_test(half_duration_timestamp_backwards_wraps),
		cmocka_unit_test(half_duration_first_frame),
		cmocka_unit_test(first_frame_half_duration_hold),
		cmocka_unit_test(buffered_offset),
		cmocka_unit_test(buffered_offset_wraps),
		cmocka_unit_test(reanchor_without_offset),
		cmocka_unit_test(reanchor_early_frame),
		cmocka_unit_test(reanchor_late_by_field_or_tick),
		cmocka_unit_test(reanchor_wraps),
		cmocka_unit_test(seq_unbuffered_25_and_50),
		cmocka_unit_test(seq_unbuffered_ntsc),
		cmocka_unit_test(seq_unbuffered_late_frame),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
