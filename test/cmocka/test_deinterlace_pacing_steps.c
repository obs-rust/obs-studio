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
 *   or, with no usable previous timestamp (none yet, a timestamp going
 *   backwards, a pause or a new clock), the half duration before when it is
 *   sane, else half the canvas interval (#1965);
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
#define HALF(ts, prev_ts, prev_half, interval, expected)                                                            \
	check_u32(obs_source_deinterlace_pacing_half_duration((ts), (prev_ts), (prev_half), (interval)), (expected), \
		  "half_duration", __LINE__)
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
	HALF(TS, obs_source_deinterlace_pacing_waiting_frame_ts(TS, TS + FRAME_50I), 0, TICK_60, FIELD_50I);
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

/* Half the time from the frame before, when there is one: then the half
 * duration before and the canvas interval are not used. */
static void half_duration_of_a_frame(void **state)
{
	(void)state;
	HALF(TS + FRAME_50I, TS, 0, TICK_60, FIELD_50I);
	HALF(TS + FRAME_50I, TS, 8 * MS, TICK_30, FIELD_50I);
	/* 59.94i: 1001/30000 s frames */
	HALF(TS + 33366667, TS, FIELD_50I, TICK_60, 16683333);
	/* rounds down */
	HALF(TS + 1, TS, FIELD_50I, TICK_60, 0);
	HALF(TS + 3, TS, FIELD_50I, TICK_60, 1);
	/* 12.5 frames a second (25 fields): 80 ms frames, 40 ms fields */
	HALF(TS + 80 * MS, TS, FIELD_50I, TICK_60, 40 * MS);
	HALF(TS + 80 * MS, TS, 0, TICK_30, 40 * MS);
	/* the longest gap that is a frame duration: twice TS_JUMP_THRESHOLD, a
	 * half duration of TS_JUMP_THRESHOLD, the most the one before may be */
	HALF(TS + 2 * JUMP, TS, FIELD_50I, TICK_60, JUMP);
	/* a frame at 1 after one at 0 is not timed: 0 is no previous frame */
	HALF(1, 0, FIELD_50I, TICK_60, FIELD_50I);
}

/* A gap past twice TS_JUMP_THRESHOLD (a pause, a new clock) is no frame
 * duration:
 * the half duration before is kept, or half the canvas interval when there
 * is none (#1965: half the gap, truncated to 32 bits, was 705 ms after a 10 s
 * pause, and 0 after 2^33 ns). */
static void half_duration_after_a_pause(void **state)
{
	(void)state;
	HALF(TS + 2 * JUMP + 1, TS, FIELD_50I, TICK_60, FIELD_50I);
	HALF(TS + 10000000000ULL, TS, FIELD_50I, TICK_60, FIELD_50I);
	HALF(TS + 2 * ((uint64_t)UINT32_MAX + 1), TS, FIELD_50I, TICK_60, FIELD_50I);
	HALF(UINT64_MAX, 1, 16683333, TICK_60, 16683333);
	/* no half duration before: half the canvas interval */
	HALF(TS + 10000000000ULL, TS, 0, TICK_60, TICK_60 / 2);
	HALF(TS + 10000000000ULL, TS, 0, TICK_30, TICK_30 / 2);
}

/* A frame at or before the one it follows (a timestamp going backwards, or
 * repeated) has no duration either (#1965: the wrapped difference gave 2^32 -
 * 20 ms for a frame 40 ms back). */
static void half_duration_timestamp_backwards(void **state)
{
	(void)state;
	HALF(TS - FRAME_50I, TS, FIELD_50I, TICK_60, FIELD_50I);
	HALF(TS - 1, TS, FIELD_50I, TICK_60, FIELD_50I);
	HALF(TS, TS, FIELD_50I, TICK_60, FIELD_50I);
	HALF(0, UINT64_MAX, FIELD_50I, TICK_60, FIELD_50I);
	HALF(TS - FRAME_50I, TS, 0, TICK_60, TICK_60 / 2);
}

/* The first frame, or the first after the queue was reset (last_frame_ts 0),
 * with no second frame queued has no previous one, and is timed against
 * deinterlace_frame_ts: 0 when the source never showed a frame, whose half
 * duration before is 0 too, so half the canvas interval; the last frame
 * shown, after a restart or a pause, so the half duration before. #1965: it
 * was half the gap, truncated to 32 bits: half the frame's own timestamp for
 * a new source (1.78 s for a capture timestamp 1000 s after boot). */
static void half_duration_first_frame(void **state)
{
	(void)state;
	/* a capture timestamp on the system clock, 1000 s after boot */
	HALF(VT, 0, 0, TICK_60, TICK_60 / 2);
	HALF(VT, 0, 0, TICK_30, TICK_30 / 2);
	/* a media timestamp of 5 s */
	HALF(TS, 0, 0, TICK_60, TICK_60 / 2);
	/* a capture timestamp that left 30 ms */
	HALF(2 * (100 * ((uint64_t)UINT32_MAX + 1) + 30 * MS), 0, 0, TICK_60, TICK_60 / 2);
	/* a first frame at 0, and one at 40 ms */
	HALF(0, 0, 0, TICK_60, TICK_60 / 2);
	HALF(FRAME_50I, 0, 0, TICK_60, TICK_60 / 2);
	/* after a restart (a new clock, behind or ahead) or a 10 s pause: the
	 * half duration from before */
	HALF(TS, VT, FIELD_50I, TICK_60, FIELD_50I);
	HALF(VT, TS, FIELD_50I, TICK_60, FIELD_50I);
	HALF(TS + 10000000000ULL, TS, FIELD_50I, TICK_60, FIELD_50I);
}

/* The half duration before is kept only when it is set and at most
 * TS_JUMP_THRESHOLD; else half the canvas interval, truncated to 32 bits. */
static void half_duration_fallback(void **state)
{
	(void)state;
	HALF(VT, 0, 1, TICK_60, 1);
	HALF(VT, 0, (uint32_t)JUMP, TICK_60, (uint32_t)JUMP);
	HALF(VT, 0, (uint32_t)JUMP + 1, TICK_60, TICK_60 / 2);
	HALF(VT, 0, UINT32_MAX, TICK_60, TICK_60 / 2);
	/* the 1.78 s a first frame used to get */
	HALF(VT, 0, 1783793664u, TICK_60, TICK_60 / 2);
	HALF(VT, 0, 0, 0, 0);
	HALF(VT, 0, 0, 1, 0);
	HALF(VT, 0, 0, 3, 1);
	HALF(VT, 0, 0, 2 * ((uint64_t)UINT32_MAX + 1) + 6, 3);
}

/* What the first frame's half duration does next (#1965). An unbuffered
 * source's first pick re-anchors (the offset is 0), so the frame's second
 * field comes half a canvas interval after the pick, and its end a canvas
 * interval after it, less TWOX_TOLERANCE: the next tick shows the second
 * field and moves on when a frame is there. It used to be held for 3.57 s,
 * past TS_JUMP_THRESHOLD, so the next tick moved on as if the clock had
 * jumped, without the second field; or, when the truncation left 0.5 to
 * 35.5 ms, held too long. A buffered source shows the second field at the
 * next tick too. */
static void first_frame_half_duration_hold(void **state)
{
	(void)state;
	/* a capture timestamp 5 ms before the tick, 1000 s after boot */
	uint64_t ts = VT - 5 * MS;
	uint32_t half = obs_source_deinterlace_pacing_half_duration(ts, 0, 0, TICK_60);
	check_u32(half, 8333333u, "half", __LINE__);
	uint64_t offset = obs_source_deinterlace_pacing_reanchor(0, VT, ts, half, TICK_60);
	check_u64(offset, 5 * MS, "offset", __LINE__);
	check_bool(VT >= ts + offset + half - TWOX, false, "second field at the pick", __LINE__);
	check_bool(VT + TICK_60 >= ts + offset + half - TWOX, true, "second field at the next tick", __LINE__);
	HOLD(ts, offset, half, VT, true);
	HOLD(ts, offset, half, VT + TICK_60 - TWOX - 1, true);
	HOLD(ts, offset, half, VT + TICK_60 - TWOX, false);
	HOLD(ts, offset, half, VT + TICK_60, false);

	/* the timestamp that left 30 ms: no longer held to 59 ms */
	ts = 2 * (100 * ((uint64_t)UINT32_MAX + 1) + 30 * MS);
	half = obs_source_deinterlace_pacing_half_duration(ts, 0, 0, TICK_60);
	offset = obs_source_deinterlace_pacing_reanchor(0, VT, ts, half, TICK_60);
	check_u64(offset, VT - ts, "offset", __LINE__);
	HOLD(ts, offset, half, VT + TICK_60, false);

	/* buffered: the offset is from last_frame_ts, the frame's own
	 * timestamp at a first pick */
	ts = VT - 5 * MS;
	half = obs_source_deinterlace_pacing_half_duration(ts, 0, 0, TICK_60);
	offset = obs_source_deinterlace_pacing_buffered_offset(VT, ts);
	check_bool(VT >= ts + offset + half - TWOX, false, "buffered second field at the pick", __LINE__);
	check_bool(VT + TICK_60 >= ts + offset + half - TWOX, true, "buffered second field at the next tick", __LINE__);

	/* after a 10 s pause, the 50i half duration from before: the second
	 * field 19 ms after the pick, the end 39 ms after it */
	ts = TS + 10000000000ULL;
	half = obs_source_deinterlace_pacing_half_duration(ts, TS, (uint32_t)FIELD_50I, TICK_60);
	check_u32(half, FIELD_50I, "half", __LINE__);
	offset = obs_source_deinterlace_pacing_reanchor(VT - TS, VT, ts, half, TICK_60);
	check_u64(offset, VT - ts, "offset", __LINE__);
	HOLD(ts, offset, half, VT + 39 * MS - 1, true);
	HOLD(ts, offset, half, VT + 39 * MS, false);
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

#define SEQ_MAX_PICKS 128

struct seq_pick {
	uint64_t tick, ts, offset;
	uint32_t half;
};

struct seq_result {
	size_t reanchors; /* offsets changed after the first pick */
	size_t skipped;   /* frames never picked */
	size_t early;     /* picks before the frame before had ended */
	/* the ticks that showed the first frame's first and second field (0:
	 * none) */
	uint64_t first_field1, first_field2;
	size_t picks;
	struct seq_pick pick[SEQ_MAX_PICKS];
};

/* One pick of the model: the newest frame that has arrived, if it is not the
 * one shown. */
static void seq_pick_next(struct seq_result *r, int64_t *last, int64_t count, uint64_t tick, uint64_t video_time,
			  uint64_t interval, uint64_t first_tick, uint64_t frame_num, uint64_t frame_den,
			  bool from_start, uint64_t *frame_ts, uint64_t *offset, uint32_t *half)
{
	int64_t next = *last;
	while (next + 1 < count && first_tick + rate_time((uint64_t)(next + 1), frame_num, frame_den) <= video_time)
		next++;
	if (next == *last)
		return;

	const uint64_t ts = TS + rate_time((uint64_t)next, frame_num, frame_den);
	uint64_t prev_ts = from_start ? 0 : ts - rate_time(1, frame_num, frame_den);
	if (*last >= 0) {
		r->skipped += (size_t)(next - *last - 1);
		if (video_time < *frame_ts + *offset + 2 * (uint64_t)*half - TWOX)
			r->early++;
		prev_ts = *frame_ts;
	}
	*half = obs_source_deinterlace_pacing_half_duration(ts, prev_ts, *half, interval);
	*frame_ts = ts;
	const uint64_t before = *offset;
	*offset = obs_source_deinterlace_pacing_reanchor(*offset, video_time, *frame_ts, *half, interval);
	if (*last >= 0 && *offset != before)
		r->reanchors++;
	if (r->picks < SEQ_MAX_PICKS)
		r->pick[r->picks++] = (struct seq_pick){tick, ts, *offset, *half};
	*last = next;
}

/* The model: an unbuffered source of frame_num/frame_den frames a second on a
 * canvas of canvas_num/canvas_den ticks a second. Frame k arrives at the first
 * tick plus k frame durations. At every tick the source keeps its frame while
 * hold() says so; else it picks the newest frame that has arrived (the
 * queue drops the older ones), takes its half duration from the frame shown
 * before and re-anchors. The first frame is timed from one frame duration
 * back, or, from_start, as a source that never showed a frame is
 * (deinterlace_frame_ts and deinterlace_half_duration 0). The ticks then
 * show the second field once video_time reaches frame_ts + offset + half -
 * TWOX_TOLERANCE, as the render does. */
static struct seq_result run_unbuffered_from(uint64_t frame_num, uint64_t frame_den, uint64_t canvas_num,
					     uint64_t canvas_den, size_t frames, bool from_start)
{
	struct seq_result r;
	memset(&r, 0, sizeof(r));
	const uint64_t interval = 1000000000ULL * canvas_den / canvas_num;
	const uint64_t first_tick = VT + rate_time(1, canvas_num, canvas_den);
	const int64_t count = (int64_t)frames;
	uint64_t frame_ts = 0;
	uint64_t offset = 0;
	uint32_t half = 0;
	int64_t last = -1; /* the frame shown, -1 before the first */

	for (uint64_t tick = 1; last + 1 < count; tick++) {
		const uint64_t video_time = VT + rate_time(tick, canvas_num, canvas_den);
		if (!(offset && obs_source_deinterlace_pacing_hold(frame_ts, offset, half, video_time)))
			seq_pick_next(&r, &last, count, tick, video_time, interval, first_tick, frame_num, frame_den,
				      from_start, &frame_ts, &offset, &half);
		if (last == 0) {
			const bool second = video_time >= frame_ts + offset + half - TWOX;
			uint64_t *field = second ? &r.first_field2 : &r.first_field1;
			if (!*field)
				*field = tick;
		}
	}
	return r;
}

static void check_seq(uint64_t frame_num, uint64_t frame_den, uint64_t canvas_num, uint64_t canvas_den,
		      size_t reanchors, size_t skipped, int line)
{
	const struct seq_result r = run_unbuffered_from(frame_num, frame_den, canvas_num, canvas_den, 100, false);
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

/* A source that never showed a frame (#1965): its first frame shows its first
 * field at the pick and its second at the next tick, before the next frame,
 * when a frame lasts longer than a tick; when it does not (50 and 59.94
 * frames a second into 30 fps), the next frame replaces it at the next tick,
 * as every frame after it is. From the second frame on, every pick (its
 * tick, timestamp, offset and half duration) is the one of a source whose
 * first frame was timed from a frame before it. It used to be timed from
 * half its own timestamp (2.5 s here): the second field never came, and the
 * second pick counted as early (the first frame "ended" seconds later). */
static void check_first_frame(uint64_t frame_num, uint64_t frame_den, uint64_t canvas_num, uint64_t canvas_den,
			      int line)
{
	struct seq_result *timed = malloc(sizeof(*timed));
	struct seq_result *start = malloc(sizeof(*start));
	*timed = run_unbuffered_from(frame_num, frame_den, canvas_num, canvas_den, 100, false);
	*start = run_unbuffered_from(frame_num, frame_den, canvas_num, canvas_den, 100, true);
	const bool frame_over_a_tick = frame_den * canvas_num > frame_num * canvas_den;

	check_u64(start->early, 0, "early picks", line);
	check_u64(start->skipped, timed->skipped, "skipped frames", line);
	check_u64(start->reanchors, timed->reanchors, "re-anchors", line);
	check_u64(start->picks, timed->picks, "picks", line);
	check_u64(start->first_field1, start->pick[0].tick, "first field", line);
	check_u64(start->first_field2, frame_over_a_tick ? start->pick[0].tick + 1 : 0, "second field", line);
	if (frame_over_a_tick)
		check_bool(start->first_field2 < start->pick[1].tick, true, "second field before the next frame", line);
	for (size_t i = 1; i < start->picks && i < timed->picks; i++) {
		check_u64(start->pick[i].tick, timed->pick[i].tick, "pick tick", line);
		check_u64(start->pick[i].ts, timed->pick[i].ts, "pick timestamp", line);
		check_u64(start->pick[i].offset, timed->pick[i].offset, "pick offset", line);
		check_u32(start->pick[i].half, timed->pick[i].half, "pick half duration", line);
	}
	free(timed);
	free(start);
}

static void seq_unbuffered_first_frame(void **state)
{
	(void)state;
	/* 12.5 frames a second (25 fields), 80 ms apart */
	check_first_frame(25, 2, 30, 1, __LINE__);
	check_first_frame(25, 2, 60, 1, __LINE__);
	check_first_frame(25, 1, 30, 1, __LINE__);
	check_first_frame(25, 1, 60, 1, __LINE__);
	check_first_frame(30000, 1001, 30, 1, __LINE__);
	check_first_frame(30000, 1001, 60, 1, __LINE__);
	check_first_frame(50, 1, 30, 1, __LINE__);
	check_first_frame(50, 1, 60, 1, __LINE__);
	check_first_frame(60000, 1001, 30, 1, __LINE__);
	check_first_frame(60000, 1001, 60, 1, __LINE__);
}

/* 12.5 frames a second (25 fields), 80 ms apart, into 30 and 60 fps: each
 * frame is timed from the one before, as before #1965 (the gap is within
 * twice TS_JUMP_THRESHOLD), so the offset holds and no frame is dropped. */
static void seq_unbuffered_12_5(void **state)
{
	(void)state;
	check_seq(25, 2, 30, 1, 0, 0, __LINE__);
	check_seq(25, 2, 60, 1, 0, 0, __LINE__);
	for (uint64_t canvas = 30; canvas <= 60; canvas += 30) {
		struct seq_result *r = malloc(sizeof(*r));
		*r = run_unbuffered_from(25, 2, canvas, 1, 100, false);
		for (size_t i = 1; i < r->picks; i++)
			check_u32(r->pick[i].half, 40 * MS, "half duration", __LINE__);
		free(r);
	}
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
		cmocka_unit_test(half_duration_after_a_pause),
		cmocka_unit_test(half_duration_timestamp_backwards),
		cmocka_unit_test(half_duration_first_frame),
		cmocka_unit_test(half_duration_fallback),
		cmocka_unit_test(first_frame_half_duration_hold),
		cmocka_unit_test(buffered_offset),
		cmocka_unit_test(buffered_offset_wraps),
		cmocka_unit_test(reanchor_without_offset),
		cmocka_unit_test(reanchor_early_frame),
		cmocka_unit_test(reanchor_late_by_field_or_tick),
		cmocka_unit_test(reanchor_wraps),
		cmocka_unit_test(seq_unbuffered_25_and_50),
		cmocka_unit_test(seq_unbuffered_ntsc),
		cmocka_unit_test(seq_unbuffered_12_5),
		cmocka_unit_test(seq_unbuffered_first_frame),
		cmocka_unit_test(seq_unbuffered_late_frame),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
