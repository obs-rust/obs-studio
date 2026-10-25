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
#include "obs-source-deinterlace-select.h"

/*
 * Characterization of the deinterlace frame selection of first_frame(),
 * ready_deinterlace_frames(), deinterlace_get_closest_frames() and
 * deinterlace_update_async_video() in libobs/obs-source-deinterlace.c, moved
 * to libobs/obs-source-deinterlace-select.c before its Rust port:
 * - obs_source_deinterlace_select_first(): a source with no last_frame_ts
 *   takes its front frame, paired when two or more are queued;
 * - obs_source_deinterlace_select_walk(): an unbuffered source drops all but
 *   two frames; a buffered one resets on a timestamp out of bounds, else
 *   walks the queue on its own clock: it stops within FRAME_TOLERANCE of the
 *   next frame, drops frame by frame behind a taken pair, stops with two
 *   frames or fewer left (the front one alone, #2009), and re-anchors its
 *   clock on a step of more than MAX_TS_VAR;
 * - obs_source_deinterlace_select_unbuffered_pair(): the "don't skip
 *   ahead" guard of an unbuffered source left with two frames;
 * - obs_source_deinterlace_select_take_*(): whether the taken frame's flag is
 *   read, whether it takes a pair, whether last_frame_ts starts there, and
 *   which offset is set;
 * - obs_source_deinterlace_select_update(): upload, swap or nothing.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. The step_* cases copy the callers from
 * obs-source-deinterlace.c onto a stand-in source and record every erase,
 * release and prev_frame write, so the composition is pinned too.
 * test_deinterlace_2x_field_order, test_media_restart_stale_frame and
 * test_media_restart_first_frame cover the callers through the real libobs.
 *
 * Frame timestamps come from capture devices and media files, so any u64 can
 * arrive; the C relies on uint64_t wrapping.
 */

#define MS 1000000ULL
#define FRAME_TOL OBS_SOURCE_DEINTERLACE_PACING_FRAME_TOLERANCE
#define TWOX OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE
#define MAX_VAR OBS_SOURCE_DEINTERLACE_SELECT_MAX_TS_VAR
/* a frame timestamp on the source's clock */
#define T 1000000000000ULL
/* 50 frames a second */
#define S (20 * MS)
/* the previous tick (last_sys_timestamp) */
#define SYS 7000000000000ULL
/* MAX_ASYNC_FRAMES of obs-source.c */
#define MAX_ASYNC 30

static void check_u64(uint64_t actual, uint64_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %" PRIu64 ", expected %" PRIu64 "\n", line, what, actual, expected);
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

static void check_str(const char *actual, const char *expected, int line)
{
	if (strcmp(actual, expected) != 0) {
		fprintf(stderr, "line %d: steps are\n  \"%s\", expected\n  \"%s\"\n", line, actual, expected);
		fail();
	}
}

/* frames at T, T + S, T + 2S, ... */
static void regular(uint64_t *ts, size_t n)
{
	for (size_t i = 0; i < n; i++)
		ts[i] = T + i * S;
}

struct walk_expect {
	size_t drop_front;
	uint64_t last_frame_ts;
	bool offset_reset;
	bool pairs;
	bool flag_front;
	bool ready;
};

static void check_walk(struct obs_source_deinterlace_select_walk_result r, struct walk_expect e, int line)
{
	check_u64(r.drop_front, e.drop_front, "drop_front", line);
	check_u64(r.last_frame_ts, e.last_frame_ts, "last_frame_ts", line);
	check_bool(r.offset_reset, e.offset_reset, "offset_reset", line);
	check_bool(r.pairs, e.pairs, "pairs", line);
	check_bool(r.flag_front, e.flag_front, "flag_front", line);
	check_bool(r.ready, e.ready, "ready", line);
}

/* A buffered walk whose clock, last_frame_ts + (sys_time -
 * last_sys_timestamp), is clock. */
#define BUFFERED(ts, n, last, clock, ...)                                                                          \
	check_walk(obs_source_deinterlace_select_walk((ts), (n), SYS + ((clock) - (last)), (last), SYS, false),     \
		   (struct walk_expect){__VA_ARGS__}, __LINE__)
#define UNBUFFERED(ts, n, ...)                                                                                   \
	check_walk(obs_source_deinterlace_select_walk((ts), (n), SYS, 12345, SYS - S, true),                      \
		   (struct walk_expect){__VA_ARGS__}, __LINE__)

/* ------------------------------------------------------------------------ */
/* enums                                                                     */

static void enum_values(void **state)
{
	(void)state;

	check_u64(OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED, 0, "OFFSET_BUFFERED", __LINE__);
	check_u64(OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR, 1, "OFFSET_REANCHOR", __LINE__);
	check_u64(sizeof(enum obs_source_deinterlace_select_offset_kind), sizeof(int), "offset kind size", __LINE__);
	check_u64(OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_NONE, 0, "UPDATE_NONE", __LINE__);
	check_u64(OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD, 1, "UPDATE_UPLOAD", __LINE__);
	check_u64(OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_SWAP, 2, "UPDATE_SWAP", __LINE__);
	check_u64(sizeof(enum obs_source_deinterlace_select_update_step), sizeof(int), "update step size", __LINE__);
	check_u64(MAX_VAR, 2000000000ULL, "MAX_TS_VAR", __LINE__);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_select_first                                       */

/* No last_frame_ts yet: the front frame is taken without the walk, and
 * flagged as the previous frame of a pair when another one follows. */
static void first_frame_with_one_and_two(void **state)
{
	(void)state;

	for (size_t count = 0; count < 5; count++) {
		struct obs_source_deinterlace_select_first_result r = obs_source_deinterlace_select_first(0, count);
		check_bool(r.first, true, "first", __LINE__);
		check_bool(r.flag_front, count >= 2, "flag_front", __LINE__);

		/* any last_frame_ts, even 1 or one that wrapped, walks */
		r = obs_source_deinterlace_select_first(1, count);
		check_bool(r.first, false, "first", __LINE__);
		check_bool(r.flag_front, false, "flag_front", __LINE__);
		r = obs_source_deinterlace_select_first(UINT64_MAX, count);
		check_bool(r.first, false, "first", __LINE__);
		check_bool(r.flag_front, false, "flag_front", __LINE__);
	}
	/* characterized, not endorsed: a frame whose timestamp is 0 leaves
	 * last_frame_ts at 0, so the next tick takes the first frame again */
	check_bool(obs_source_deinterlace_select_first(0, SIZE_MAX).flag_front, true, "flag_front", __LINE__);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_select_walk: unbuffered                            */

/* All but two frames are dropped; the front one becomes the source clock,
 * and with two left the guard decides the pair. Past MAX_ASYNC_FRAMES too. */
static void unbuffered_sizes(void **state)
{
	(void)state;
	uint64_t ts[40];
	regular(ts, 40);

	/* nothing queued: nothing to do, last_frame_ts kept */
	UNBUFFERED(NULL, 0, .last_frame_ts = 12345);
	UNBUFFERED(ts, 0, .last_frame_ts = 12345);
	UNBUFFERED(ts, 1, .last_frame_ts = T, .ready = true);
	UNBUFFERED(ts, 2, .last_frame_ts = T, .pairs = true, .ready = true);
	UNBUFFERED(ts, 3, .drop_front = 1, .last_frame_ts = T + S, .pairs = true, .ready = true);
	UNBUFFERED(ts, 4, .drop_front = 2, .last_frame_ts = T + 2 * S, .pairs = true, .ready = true);
	UNBUFFERED(ts, 5, .drop_front = 3, .last_frame_ts = T + 3 * S, .pairs = true, .ready = true);
	UNBUFFERED(ts, MAX_ASYNC, .drop_front = MAX_ASYNC - 2, .last_frame_ts = T + (MAX_ASYNC - 2) * S, .pairs = true,
		   .ready = true);
	UNBUFFERED(ts, 33, .drop_front = 31, .last_frame_ts = T + 31 * S, .pairs = true, .ready = true);
}

/* The clock, the timestamps' order and the state play no part. */
static void unbuffered_ignores_clock(void **state)
{
	(void)state;
	uint64_t ts[4] = {UINT64_MAX, 0, 5, 3};

	check_walk(obs_source_deinterlace_select_walk(ts, 4, 0, 0, UINT64_MAX, true),
		   (struct walk_expect){.drop_front = 2, .last_frame_ts = 5, .pairs = true, .ready = true}, __LINE__);
	check_walk(obs_source_deinterlace_select_walk(ts, 1, UINT64_MAX, T, 0, true),
		   (struct walk_expect){.last_frame_ts = UINT64_MAX, .ready = true}, __LINE__);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_select_walk: buffered                              */

static void buffered_empty(void **state)
{
	(void)state;

	check_walk(obs_source_deinterlace_select_walk(NULL, 0, SYS, T, SYS, false),
		   (struct walk_expect){.last_frame_ts = T}, __LINE__);
}

/* The front frame more than MAX_TS_VAR from the clock, either way: the
 * clock restarts at it, the offset resets and the front frame is taken. */
static void buffered_out_of_bounds(void **state)
{
	(void)state;
	uint64_t ts[3];
	regular(ts, 3);

	/* clock behind the frame */
	BUFFERED(ts, 3, T - MAX_VAR - 1, T - MAX_VAR - 1, .last_frame_ts = T, .offset_reset = true, .ready = true);
	/* clock ahead of it */
	BUFFERED(ts, 3, T + MAX_VAR + 1, T + MAX_VAR + 1, .last_frame_ts = T, .offset_reset = true, .ready = true);
	/* at MAX_TS_VAR it is in bounds: the clock moves on and nothing is
	 * due yet */
	BUFFERED(ts, 3, T - MAX_VAR, T - MAX_VAR + 5 * MS, .last_frame_ts = T - MAX_VAR + 5 * MS);
	BUFFERED(ts, 3, T - MAX_VAR + 1, T - MAX_VAR + 1, .last_frame_ts = T - MAX_VAR + 1);
	/* ahead by MAX_TS_VAR: in bounds, far past every frame, so it drops to
	 * the last two */
	BUFFERED(ts, 3, T + MAX_VAR, T + MAX_VAR, .drop_front = 1, .last_frame_ts = T + MAX_VAR, .offset_reset = true,
		 .ready = true);
	/* the check uses last_frame_ts before the clock moves */
	BUFFERED(ts, 1, T - MAX_VAR - 1, T, .last_frame_ts = T, .offset_reset = true, .ready = true);
}

/* A frame is due once the clock is FRAME_TOLERANCE past it. */
static void buffered_frame_tolerance(void **state)
{
	(void)state;
	uint64_t ts[3];
	regular(ts, 3);

	/* before the front frame: nothing taken */
	BUFFERED(ts, 3, T - 10 * MS, T - 5 * MS, .last_frame_ts = T - 5 * MS);
	BUFFERED(ts, 3, T - 10 * MS, T, .last_frame_ts = T);
	/* past it by less than the tolerance: still nothing */
	BUFFERED(ts, 3, T - 10 * MS, T + FRAME_TOL - 1, .last_frame_ts = T + FRAME_TOL - 1);
	/* at it and past it: the front frame is taken alone */
	BUFFERED(ts, 3, T - 10 * MS, T + FRAME_TOL, .last_frame_ts = T + FRAME_TOL, .ready = true);
	BUFFERED(ts, 3, T - 10 * MS, T + FRAME_TOL + 1, .last_frame_ts = T + FRAME_TOL + 1, .ready = true);
	/* the next frame, within the tolerance: the front one alone still */
	BUFFERED(ts, 3, T, T + S + FRAME_TOL - 1, .last_frame_ts = T + S + FRAME_TOL - 1, .ready = true);
	/* the next frame due too: the front frame is its pair */
	uint64_t five[5];
	regular(five, 5);
	BUFFERED(five, 5, T, T + S + FRAME_TOL, .last_frame_ts = T + S + FRAME_TOL, .flag_front = true, .ready = true);
}

/* Behind a taken pair the walk drops the front frame each step, until the
 * clock is within the tolerance of the next frame. */
static void buffered_drops_pairwise(void **state)
{
	(void)state;
	uint64_t ts[40];
	regular(ts, 40);

	/* frames 2 and 3 are due: 0 and 1 are dropped, 2 pairs with 3 */
	BUFFERED(ts, 5, T, T + 3 * S + 5 * MS, .drop_front = 2, .last_frame_ts = T + 3 * S + 5 * MS, .flag_front = true,
		 .ready = true);
	BUFFERED(ts, 6, T, T + 3 * S + 5 * MS, .drop_front = 2, .last_frame_ts = T + 3 * S + 5 * MS, .flag_front = true,
		 .ready = true);
	BUFFERED(ts, MAX_ASYNC, T, T + 3 * S + 5 * MS, .drop_front = 2, .last_frame_ts = T + 3 * S + 5 * MS,
		 .flag_front = true, .ready = true);
	BUFFERED(ts, 33, T, T + 3 * S + 5 * MS, .drop_front = 2, .last_frame_ts = T + 3 * S + 5 * MS,
		 .flag_front = true, .ready = true);
	BUFFERED(ts, 33, T, T + 29 * S + 5 * MS, .drop_front = 28, .last_frame_ts = T + 29 * S + 5 * MS,
		 .flag_front = true, .ready = true);
}

/* Two frames or fewer left: the walk stops, the offset resets and the front
 * frame is shown alone, unless nothing was taken yet (then a second frame
 * lets it go on). A frame just dropped is no pair for it (#2009). */
static void buffered_two_or_fewer_exit(void **state)
{
	(void)state;
	uint64_t ts[40];
	regular(ts, 40);
	const uint64_t far = T + 100 * S;

	/* one frame, nothing taken: the exit */
	BUFFERED(ts, 1, T, far, .last_frame_ts = far, .offset_reset = true, .ready = true);
	/* two frames, nothing taken: the exception takes the first, then the
	 * exit with it taken */
	BUFFERED(ts, 2, T, far, .last_frame_ts = far, .offset_reset = true, .ready = true);
	/* three: the first is dropped behind the second, which is left alone
	 * with the third (#2009: no flag) */
	BUFFERED(ts, 3, T, far, .drop_front = 1, .last_frame_ts = far, .offset_reset = true, .ready = true);
	BUFFERED(ts, 4, T, far, .drop_front = 2, .last_frame_ts = far, .offset_reset = true, .ready = true);
	BUFFERED(ts, 5, T, far, .drop_front = 3, .last_frame_ts = far, .offset_reset = true, .ready = true);
	BUFFERED(ts, MAX_ASYNC, T, far, .drop_front = MAX_ASYNC - 2, .last_frame_ts = far, .offset_reset = true,
		 .ready = true);
	BUFFERED(ts, 33, T, far, .drop_front = 31, .last_frame_ts = far, .offset_reset = true, .ready = true);
	/* four frames with the fourth not due: the exit comes first */
	BUFFERED(ts, 4, T, T + 3 * S + 5 * MS, .drop_front = 2, .last_frame_ts = T + 3 * S + 5 * MS,
		 .offset_reset = true, .ready = true);
	/* two frames with the second within the tolerance: no exit */
	BUFFERED(ts, 2, T, T + S + FRAME_TOL - 1, .last_frame_ts = T + S + FRAME_TOL - 1, .ready = true);
}

/* A step of more than MAX_TS_VAR to the next frame re-anchors the clock on
 * it, keeping the front frame's offset from the clock, and resets the
 * offset. */
static void buffered_max_ts_var(void **state)
{
	(void)state;
	uint64_t ts[3];

	for (int d = -1; d <= 1; d++) {
		const uint64_t gap = MAX_VAR + (uint64_t)(int64_t)d;
		ts[0] = T;
		ts[1] = T + gap;
		ts[2] = T + gap + S;
		if (d > 0)
			/* the clock jumps to the next frame (offset 0 from the
			 * front one), which is not due on it */
			BUFFERED(ts, 3, T, T + 5 * MS, .last_frame_ts = T + gap, .offset_reset = true, .ready = true);
		else
			BUFFERED(ts, 3, T, T + 5 * MS, .last_frame_ts = T + 5 * MS, .ready = true);
	}

	/* the front frame 3 ms behind the clock: the anchor keeps that, so the
	 * next frame is due on the new clock too, and the front frame pairs with
	 * it across the jump */
	ts[0] = T;
	ts[1] = T + MAX_VAR + 1;
	ts[2] = T + MAX_VAR + 1 + S;
	BUFFERED(ts, 3, T + 3 * MS, T + 8 * MS, .last_frame_ts = T + MAX_VAR + 1 + 3 * MS, .offset_reset = true,
		 .flag_front = true, .ready = true);
}

/* Characterized, not endorsed: the u64 differences wrap. A timestamp going
 * backwards is a step of almost 2^64, so it re-anchors; sys_time before
 * last_sys_timestamp moves the clock back; and the compares do not wrap, so
 * frames past UINT64_MAX are far behind a clock that has not wrapped. */
static void buffered_timestamps_backwards(void **state)
{
	(void)state;
	uint64_t ts[3] = {T, T - S, T - 2 * S};

	BUFFERED(ts, 3, T, T + 5 * MS, .last_frame_ts = T - S, .offset_reset = true, .ready = true);

	/* sys_time one ns behind last_sys_timestamp: the clock goes one ns
	 * back, nothing is due */
	uint64_t fwd[3];
	regular(fwd, 3);
	check_walk(obs_source_deinterlace_select_walk(fwd, 3, SYS - 1, T - 10 * MS, SYS, false),
		   (struct walk_expect){.last_frame_ts = T - 10 * MS - 1}, __LINE__);
	/* the timestamps wrap past UINT64_MAX (the step is small, so no
	 * re-anchor) and the clock with them, 15 ms short of it: the wrapped
	 * frames compare as far behind, so the walk drops to the last two */
	uint64_t wrap[3] = {UINT64_MAX - S + 1, 0, S};
	check_walk(obs_source_deinterlace_select_walk(wrap, 3, SYS + 5 * MS, UINT64_MAX - S + 1, SYS, false),
		   (struct walk_expect){.drop_front = 1,
					.last_frame_ts = UINT64_MAX - S + 1 + 5 * MS,
					.offset_reset = true,
					.ready = true},
		   __LINE__);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_select_unbuffered_pair                             */

static void check_pair(struct obs_source_deinterlace_select_pair_result r, bool prev_frame, bool set_frame_ts,
		       uint64_t frame_ts, int line)
{
	check_bool(r.prev_frame, prev_frame, "prev_frame", line);
	check_bool(r.set_frame_ts, set_frame_ts, "set_frame_ts", line);
	check_u64(r.frame_ts, frame_ts, "frame_ts", line);
}

/* Without an offset the two frames pair. With one, the front frame waits
 * alone while sys_time is before its end (timestamp + offset + its
 * duration), from a frame_ts one duration back. */
static void unbuffered_pair_guard(void **state)
{
	(void)state;
	const uint64_t offset = SYS - T;

	check_pair(obs_source_deinterlace_select_unbuffered_pair(T, T + S, 0, SYS), true, false, 0, __LINE__);
	check_pair(obs_source_deinterlace_select_unbuffered_pair(T, T + S, 0, 0), true, false, 0, __LINE__);
	check_pair(obs_source_deinterlace_select_unbuffered_pair(T, T + S, offset, SYS), false, true, T - S, __LINE__);
	check_pair(obs_source_deinterlace_select_unbuffered_pair(T, T + S, offset, SYS + S - 1), false, true, T - S,
		   __LINE__);
	check_pair(obs_source_deinterlace_select_unbuffered_pair(T, T + S, offset, SYS + S), true, false, 0, __LINE__);
	check_pair(obs_source_deinterlace_select_unbuffered_pair(T, T + S, offset, UINT64_MAX), true, false, 0,
		   __LINE__);
	/* characterized, not endorsed: the next frame before the first wraps
	 * the duration, so the end wraps to just before timestamp + offset */
	check_pair(obs_source_deinterlace_select_unbuffered_pair(T, T - S, offset, SYS - S - 1), false, true, T + S,
		   __LINE__);
	check_pair(obs_source_deinterlace_select_unbuffered_pair(T, T - S, offset, SYS - S), true, false, 0, __LINE__);
}

/* ------------------------------------------------------------------------ */
/* obs_source_deinterlace_select_take_* and _update                          */

static void take_decisions(void **state)
{
	(void)state;

	check_bool(obs_source_deinterlace_select_take_reads_flag(0), false, "reads_flag", __LINE__);
	check_bool(obs_source_deinterlace_select_take_reads_flag(1), true, "reads_flag", __LINE__);
	check_bool(obs_source_deinterlace_select_take_reads_flag(SIZE_MAX), true, "reads_flag", __LINE__);

	check_bool(obs_source_deinterlace_select_take_pair(0, false), false, "pair", __LINE__);
	check_bool(obs_source_deinterlace_select_take_pair(0, true), false, "pair", __LINE__);
	check_bool(obs_source_deinterlace_select_take_pair(1, false), false, "pair", __LINE__);
	check_bool(obs_source_deinterlace_select_take_pair(1, true), true, "pair", __LINE__);
	check_bool(obs_source_deinterlace_select_take_pair(SIZE_MAX, true), true, "pair", __LINE__);

	check_bool(obs_source_deinterlace_select_take_init_last(0), true, "init_last", __LINE__);
	check_bool(obs_source_deinterlace_select_take_init_last(1), false, "init_last", __LINE__);
	check_bool(obs_source_deinterlace_select_take_init_last(UINT64_MAX), false, "init_last", __LINE__);

	check_u64(obs_source_deinterlace_select_take_offset(false), OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED,
		  "take_offset", __LINE__);
	check_u64(obs_source_deinterlace_select_take_offset(true), OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR,
		  "take_offset", __LINE__);
}

static void update_cases(void **state)
{
	(void)state;

	check_u64(obs_source_deinterlace_select_update(false, false), OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_NONE,
		  "update", __LINE__);
	check_u64(obs_source_deinterlace_select_update(false, true), OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_SWAP,
		  "update", __LINE__);
	check_u64(obs_source_deinterlace_select_update(true, false), OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD,
		  "update", __LINE__);
	check_u64(obs_source_deinterlace_select_update(true, true), OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD,
		  "update", __LINE__);
}

/* ------------------------------------------------------------------------ */
/* The callers, copied from obs-source-deinterlace.c onto a stand-in source  */

struct frame {
	int id;
	uint64_t timestamp;
	bool prev_frame;
};

struct source {
	struct frame *queue[40];
	size_t num;
	uint64_t last_frame_ts;
	uint64_t last_sys_timestamp;
	bool async_unbuffered;
	uint64_t deinterlace_offset;
	uint64_t deinterlace_frame_ts;
	uint32_t deinterlace_half_duration;
	struct frame *prev_async_frame;
	struct frame *cur_async_frame;
	char steps[1024];
};

static struct {
	uint64_t video_time;
	uint64_t video_frame_interval_ns;
} video = {SYS + 3 * MS, 16666667};

static void step(struct source *s, const char *fmt, ...)
{
	size_t len = strlen(s->steps);
	va_list args;
	va_start(args, fmt);
	vsnprintf(s->steps + len, sizeof(s->steps) - len, fmt, args);
	va_end(args);
}

static void erase_front(struct source *s)
{
	step(s, "E%d ", s->queue[0]->id);
	memmove(s->queue, s->queue + 1, (s->num - 1) * sizeof(*s->queue));
	s->num--;
}

static void remove_async_frame(struct source *s, struct frame *frame)
{
	step(s, "R%d ", frame->id);
	frame->prev_frame = false;
}

static void set_flag(struct source *s, struct frame *frame, bool flag)
{
	step(s, "F%d=%d ", frame->id, flag);
	frame->prev_frame = flag;
}

/* ready_deinterlace_frames() */
static bool ready_frames(struct source *s, uint64_t sys_time)
{
	uint64_t stack_timestamps[MAX_ASYNC];
	uint64_t *timestamps = stack_timestamps;
	const size_t num = s->num;
	const uint64_t last_sys_timestamp = s->last_sys_timestamp;
	const bool async_unbuffered = s->async_unbuffered;
	const uint64_t last_frame_ts = async_unbuffered ? 0 : s->last_frame_ts;
	struct obs_source_deinterlace_select_walk_result walk;

	if (num > MAX_ASYNC)
		timestamps = malloc(num * sizeof(*timestamps));
	for (size_t i = 0; i < num; i++)
		timestamps[i] = s->queue[i]->timestamp;

	walk = obs_source_deinterlace_select_walk(timestamps, num, sys_time, last_frame_ts, last_sys_timestamp,
						  async_unbuffered);

	for (size_t i = 0; i < walk.drop_front; i++) {
		struct frame *dropped = s->queue[0];
		erase_front(s);
		remove_async_frame(s, dropped);
	}

	if (walk.pairs) {
		const uint64_t offset = s->async_unbuffered ? s->deinterlace_offset : 0;
		const struct obs_source_deinterlace_select_pair_result pair =
			obs_source_deinterlace_select_unbuffered_pair(
				timestamps[walk.drop_front], timestamps[walk.drop_front + 1], offset, sys_time);

		if (pair.set_frame_ts)
			s->deinterlace_frame_ts = pair.frame_ts;
		set_flag(s, s->queue[0], pair.prev_frame);
	}

	if (timestamps != stack_timestamps)
		free(timestamps);

	s->last_frame_ts = walk.last_frame_ts;
	if (walk.offset_reset)
		s->deinterlace_offset = 0;
	if (walk.flag_front)
		set_flag(s, s->queue[0], true);

	return walk.ready;
}

/* first_frame() */
static bool first_frame(struct source *s)
{
	const struct obs_source_deinterlace_select_first_result first =
		obs_source_deinterlace_select_first(s->last_frame_ts, s->num);

	if (first.flag_front)
		set_flag(s, s->queue[0], true);
	return first.first;
}

/* deinterlace_get_closest_frames() */
static void get_closest_frames(struct source *s, uint64_t sys_time)
{
	if (s->async_unbuffered && s->deinterlace_offset) {
		if (obs_source_deinterlace_pacing_hold(s->deinterlace_frame_ts, s->deinterlace_offset,
						       s->deinterlace_half_duration, sys_time)) {
			step(s, "hold");
			return;
		}
	}

	if (!s->num)
		return;

	if (first_frame(s) || ready_frames(s, sys_time)) {
		s->prev_async_frame = NULL;
		s->cur_async_frame = s->queue[0];

		erase_front(s);

		const size_t remaining = s->num;
		const bool prev_frame = obs_source_deinterlace_select_take_reads_flag(remaining) &&
					s->cur_async_frame->prev_frame;

		if (obs_source_deinterlace_select_take_pair(remaining, prev_frame)) {
			s->prev_async_frame = s->cur_async_frame;
			s->cur_async_frame = s->queue[0];

			erase_front(s);

			s->deinterlace_half_duration = obs_source_deinterlace_pacing_half_duration(
				s->cur_async_frame->timestamp, s->prev_async_frame->timestamp,
				s->deinterlace_half_duration, video.video_frame_interval_ns);
		} else {
			s->deinterlace_half_duration = obs_source_deinterlace_pacing_half_duration(
				s->cur_async_frame->timestamp, s->deinterlace_frame_ts, s->deinterlace_half_duration,
				video.video_frame_interval_ns);
		}

		if (obs_source_deinterlace_select_take_init_last(s->last_frame_ts))
			s->last_frame_ts = s->cur_async_frame->timestamp;

		s->deinterlace_frame_ts = s->cur_async_frame->timestamp;

		switch (obs_source_deinterlace_select_take_offset(s->async_unbuffered)) {
		case OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED:
			s->deinterlace_offset =
				obs_source_deinterlace_pacing_buffered_offset(video.video_time, s->last_frame_ts);
			return;
		case OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR:
		default:
			break;
		}

		s->deinterlace_offset = obs_source_deinterlace_pacing_reanchor(s->deinterlace_offset, video.video_time,
									       s->deinterlace_frame_ts,
									       s->deinterlace_half_duration,
									       video.video_frame_interval_ns);
	}
}

static struct frame frames[40];

/* n frames at T + i * step, ids 0 to n - 1, unflagged */
static void queue_frames(struct source *s, size_t n, uint64_t step_ns)
{
	memset(s, 0, sizeof(*s));
	for (size_t i = 0; i < n; i++) {
		frames[i] = (struct frame){(int)i, T + i * step_ns, false};
		s->queue[i] = &frames[i];
	}
	s->num = n;
	s->last_sys_timestamp = SYS;
}

static int fid(const struct frame *f)
{
	return f ? f->id : -1;
}

/* The first frame: one queued is shown alone; two pair (the first flagged
 * and taken as the previous frame). */
static void step_first_frame(void **state)
{
	(void)state;
	struct source s;

	queue_frames(&s, 1, S);
	get_closest_frames(&s, SYS);
	check_str(s.steps, "E0 ", __LINE__);
	check_u64(fid(s.cur_async_frame), 0, "cur", __LINE__);
	check_u64(fid(s.prev_async_frame), (uint64_t)-1, "prev", __LINE__);
	check_u64(s.last_frame_ts, T, "last_frame_ts", __LINE__);
	check_u64(s.deinterlace_frame_ts, T, "frame_ts", __LINE__);
	check_u64(s.deinterlace_offset, video.video_time - T + FRAME_TOL + TWOX, "offset", __LINE__);

	queue_frames(&s, 2, S);
	get_closest_frames(&s, SYS);
	check_str(s.steps, "F0=1 E0 E1 ", __LINE__);
	check_u64(fid(s.prev_async_frame), 0, "prev", __LINE__);
	check_u64(fid(s.cur_async_frame), 1, "cur", __LINE__);
	check_u64(s.last_frame_ts, T + S, "last_frame_ts", __LINE__);
	check_u64(s.deinterlace_half_duration, S / 2, "half_duration", __LINE__);
}

/* A buffered source two frames behind: each dropped frame is erased, then
 * released; the new front one is flagged last and pairs with the next. */
static void step_buffered_drop_and_pair(void **state)
{
	(void)state;
	struct source s;

	queue_frames(&s, 5, S);
	s.last_frame_ts = T;
	s.deinterlace_offset = 77;
	get_closest_frames(&s, SYS + 3 * S + 5 * MS);
	check_str(s.steps, "E0 R0 E1 R1 F2=1 E2 E3 ", __LINE__);
	check_u64(fid(s.prev_async_frame), 2, "prev", __LINE__);
	check_u64(fid(s.cur_async_frame), 3, "cur", __LINE__);
	check_u64(s.num, 1, "left", __LINE__);
	check_u64(s.last_frame_ts, T + 3 * S + 5 * MS, "last_frame_ts", __LINE__);
	check_u64(s.deinterlace_offset, video.video_time - (T + 3 * S + 5 * MS) + FRAME_TOL + TWOX, "offset", __LINE__);
}

/* #2009: three frames far behind: the first is dropped behind the second,
 * which is then shown alone, unflagged, with the third left queued. */
static void step_buffered_dropped_prev_is_no_pair(void **state)
{
	(void)state;
	struct source s;

	queue_frames(&s, 3, S);
	s.last_frame_ts = T;
	get_closest_frames(&s, SYS + 100 * S);
	check_str(s.steps, "E0 R0 E1 ", __LINE__);
	check_u64(fid(s.cur_async_frame), 1, "cur", __LINE__);
	check_u64(fid(s.prev_async_frame), (uint64_t)-1, "prev", __LINE__);
	check_u64(s.num, 1, "left", __LINE__);

	/* nothing due: nothing taken, the clock moves on */
	queue_frames(&s, 3, S);
	s.last_frame_ts = T - 10 * MS;
	get_closest_frames(&s, SYS + 5 * MS);
	check_str(s.steps, "", __LINE__);
	check_u64(s.num, 3, "left", __LINE__);
	check_u64(s.last_frame_ts, T - 5 * MS, "last_frame_ts", __LINE__);
}

/* An unbuffered source with 33 frames queued (past MAX_ASYNC_FRAMES, from
 * the heap copy): 31 dropped in order, the front one flagged and paired. */
static void step_unbuffered_past_max(void **state)
{
	(void)state;
	struct source s;
	char expected[1024] = "";

	queue_frames(&s, 33, S);
	s.async_unbuffered = true;
	s.last_frame_ts = T;
	get_closest_frames(&s, SYS);
	for (int i = 0; i < 31; i++) {
		size_t len = strlen(expected);
		snprintf(expected + len, sizeof(expected) - len, "E%d R%d ", i, i);
	}
	strcat(expected, "F31=1 E31 E32 ");
	check_str(s.steps, expected, __LINE__);
	check_u64(fid(s.prev_async_frame), 31, "prev", __LINE__);
	check_u64(fid(s.cur_async_frame), 32, "cur", __LINE__);
	check_u64(s.last_frame_ts, T + 31 * S, "last_frame_ts", __LINE__);
	/* no offset before: re-anchored at the current frame */
	check_u64(s.deinterlace_offset, video.video_time - (T + 32 * S), "offset", __LINE__);
}

/* An unbuffered source with an offset, its two frames' end ahead: the front
 * frame waits alone, from a frame_ts one duration back. */
static void step_unbuffered_waits(void **state)
{
	(void)state;
	struct source s;

	queue_frames(&s, 2, S);
	s.async_unbuffered = true;
	s.last_frame_ts = T - S;
	s.deinterlace_offset = SYS - T;
	s.deinterlace_frame_ts = T - 10 * S; /* long past: no hold */
	get_closest_frames(&s, SYS + 5 * MS);
	check_str(s.steps, "F0=0 E0 ", __LINE__);
	check_u64(fid(s.cur_async_frame), 0, "cur", __LINE__);
	check_u64(s.num, 1, "left", __LINE__);
	check_u64(s.last_frame_ts, T, "last_frame_ts", __LINE__);
	check_u64(s.deinterlace_frame_ts, T, "frame_ts", __LINE__);
	/* half the duration from the waiting frame_ts (T - S) */
	check_u64(s.deinterlace_half_duration, S / 2, "half_duration", __LINE__);

	/* at the end: they pair */
	queue_frames(&s, 2, S);
	s.async_unbuffered = true;
	s.last_frame_ts = T - S;
	s.deinterlace_offset = SYS - T;
	s.deinterlace_frame_ts = T - 10 * S;
	get_closest_frames(&s, SYS + S);
	check_str(s.steps, "F0=1 E0 E1 ", __LINE__);
	check_u64(fid(s.prev_async_frame), 0, "prev", __LINE__);
	check_u64(fid(s.cur_async_frame), 1, "cur", __LINE__);
}

/* deinterlace_update_async_video()'s step */
static const char *update_step(struct frame *prev, struct frame *cur)
{
	switch (obs_source_deinterlace_select_update(prev != NULL, cur != NULL)) {
	case OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD:
		return "upload";
	case OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_SWAP:
		return "swap";
	case OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_NONE:
	default:
		return "none";
	}
}

static void step_update(void **state)
{
	(void)state;
	struct frame a = {0, 0, false}, b = {1, 0, false};

	check_str(update_step(&a, &b), "upload", __LINE__);
	check_str(update_step(&a, NULL), "upload", __LINE__);
	check_str(update_step(NULL, &b), "swap", __LINE__);
	check_str(update_step(NULL, NULL), "none", __LINE__);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enum_values),
		cmocka_unit_test(first_frame_with_one_and_two),
		cmocka_unit_test(unbuffered_sizes),
		cmocka_unit_test(unbuffered_ignores_clock),
		cmocka_unit_test(buffered_empty),
		cmocka_unit_test(buffered_out_of_bounds),
		cmocka_unit_test(buffered_frame_tolerance),
		cmocka_unit_test(buffered_drops_pairwise),
		cmocka_unit_test(buffered_two_or_fewer_exit),
		cmocka_unit_test(buffered_max_ts_var),
		cmocka_unit_test(buffered_timestamps_backwards),
		cmocka_unit_test(unbuffered_pair_guard),
		cmocka_unit_test(take_decisions),
		cmocka_unit_test(update_cases),
		cmocka_unit_test(step_first_frame),
		cmocka_unit_test(step_buffered_drop_and_pair),
		cmocka_unit_test(step_buffered_dropped_prev_is_no_pair),
		cmocka_unit_test(step_unbuffered_past_max),
		cmocka_unit_test(step_unbuffered_waits),
		cmocka_unit_test(step_update),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
