#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <inttypes.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-audio-timing.h"

/*
 * Characterization of the audio timestamp placement of
 * source_output_audio_data() in libobs/obs-source.c, moved to
 * libobs/obs-source-audio-timing.c before its Rust port:
 * - obs_source_audio_timing_pre_lock(): the direct timestamp check, then
 *   reset / jump / smooth / log against next_audio_ts_min, the drift error
 *   for the resampler and the next expected timestamp;
 * - obs_source_audio_timing_under_lock(): push back or place, the reset
 *   when the system time jumped and the audio was not continuous (#522),
 *   the sync and resampler offsets, next_audio_sys_ts_min, and "the sync
 *   offset changed, so place" (#333);
 * - obs_source_audio_timing_buffer_start(): where an emptied buffer starts
 *   (#286).
 * Each case pins every output of a step, that the step writes its numeric
 * outputs (they start as a garbage byte pattern) and that it leaves its
 * inputs alone. Cases marked "characterized, not endorsed" are odd but
 * real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. The seq_* cases run several packets
 * through a fake source: the fields the steps read and write, and a model
 * of what source_output_audio_data() does around them (handle_ts_jump(),
 * and the buffer start of source_output_audio_place()). The real-libobs
 * tests (test_async_delay_audio, test_audio_sync_offset_change,
 * test_media_sync_offset_restart, test_audio_restart_resampler_tail,
 * test_audio_clock_drift) cover the caller.
 *
 * The timestamps come from sources (FFmpeg pts for media files), the sync
 * offset from scene collections and obs-websocket, so any u64 and i64 can
 * arrive.
 */

#define MS 1000000ULL
#define SMOOTH OBS_SOURCE_AUDIO_TS_SMOOTHING_THRESHOLD
#define MAX_VAR OBS_SOURCE_AUDIO_MAX_TS_VAR
/* the system clock, far from the media timestamps below */
#define OS 1000000000000000ULL
/* a media file's timestamp, more than MAX_TS_VAR from OS */
#define TS 5000000000ULL
/* one 1024-frame packet at 48 kHz */
#define DUR 21333333ULL
/* a byte pattern that no step output equals by chance */
#define GARBAGE 0xa5

#define KEEP OBS_SOURCE_AUDIO_TS_KEEP
#define RESET OBS_SOURCE_AUDIO_TS_RESET
#define JUMP OBS_SOURCE_AUDIO_TS_JUMP
#define SMOOTHED OBS_SOURCE_AUDIO_TS_SMOOTH
#define EXCEEDED OBS_SOURCE_AUDIO_TS_EXCEEDED

static const char *action_name(int action)
{
	switch (action) {
	case KEEP:
		return "KEEP";
	case RESET:
		return "RESET";
	case JUMP:
		return "JUMP";
	case SMOOTHED:
		return "SMOOTH";
	case EXCEEDED:
		return "EXCEEDED";
	}
	return "(invalid)";
}

static void check_u64(uint64_t actual, uint64_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %" PRIu64 ", expected %" PRIu64 "\n", line, what, actual, expected);
		fail();
	}
}

static void check_i64(int64_t actual, int64_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %" PRId64 ", expected %" PRId64 "\n", line, what, actual, expected);
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

/* ------------------------------------------------------------------------ */
/* obs_source_audio_timing_pre_lock                                          */

struct pre_in {
	uint64_t timestamp;
	uint64_t os_time;
	uint64_t duration;
	uint64_t next_audio_ts_min;
	bool timing_set;
	bool async_unbuffered;
	bool async_decoupled;
};

struct pre_out {
	int action;
	bool direct;
	bool reanchor;
	bool continuous;
	uint64_t timestamp;
	uint64_t next_audio_ts_min;
	uint64_t timing_adjust;
	uint64_t diff;
	int64_t drift_error;
};

static struct obs_source_audio_timing_pre run_pre(struct pre_in in)
{
	struct obs_source_audio_timing_pre t;
	memset(&t, GARBAGE, sizeof(t));
	t.timestamp = in.timestamp;
	t.os_time = in.os_time;
	t.duration = in.duration;
	t.next_audio_ts_min = in.next_audio_ts_min;
	t.timing_set = in.timing_set;
	t.async_unbuffered = in.async_unbuffered;
	t.async_decoupled = in.async_decoupled;
	/* outputs the step must write */
	t.direct = true;
	t.reanchor = true;
	t.continuous = true;
	obs_source_audio_timing_pre_lock(&t);
	return t;
}

static void check_pre_at(struct pre_in in, struct pre_out e, int line)
{
	struct obs_source_audio_timing_pre t = run_pre(in);
	if ((int)t.action != e.action) {
		fprintf(stderr, "line %d: action is %s, expected %s\n", line, action_name((int)t.action),
			action_name(e.action));
		fail();
	}
	check_bool(t.direct, e.direct, "direct", line);
	check_bool(t.reanchor, e.reanchor, "reanchor", line);
	check_bool(t.continuous, e.continuous, "continuous", line);
	check_u64(t.timestamp, e.timestamp, "timestamp", line);
	check_u64(t.next_audio_ts_min, e.next_audio_ts_min, "next_audio_ts_min", line);
	check_u64(t.timing_adjust, e.timing_adjust, "timing_adjust", line);
	check_u64(t.diff, e.diff, "diff", line);
	check_i64(t.drift_error, e.drift_error, "drift_error", line);
	/* the inputs are left alone */
	check_u64(t.os_time, in.os_time, "os_time", line);
	check_u64(t.duration, in.duration, "duration", line);
	check_bool(t.timing_set, in.timing_set, "timing_set", line);
	check_bool(t.async_unbuffered, in.async_unbuffered, "async_unbuffered", line);
	check_bool(t.async_decoupled, in.async_decoupled, "async_decoupled", line);
}

#define check_pre(in, ...) check_pre_at(in, (struct pre_out){__VA_ARGS__}, __LINE__)

/* A packet whose timestamp is within MAX_TS_VAR of the system clock is on
 * the system clock: timing_adjust 0 and timing_set, whatever timing_set
 * was. With no expected timestamp it keeps its own. */
static void pre_direct_timestamps(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = OS + 5 * MS, .os_time = OS, .duration = DUR};
	check_pre(in, .action = KEEP, .direct = true, .timestamp = OS + 5 * MS, .next_audio_ts_min = OS + 5 * MS + DUR);
	in.timing_set = true;
	check_pre(in, .action = KEEP, .direct = true, .timestamp = OS + 5 * MS, .next_audio_ts_min = OS + 5 * MS + DUR);
	/* behind the clock too */
	in.timestamp = OS - 1500 * MS;
	check_pre(in, .action = KEEP, .direct = true, .timestamp = OS - 1500 * MS,
		  .next_audio_ts_min = OS - 1500 * MS + DUR);
	/* equal */
	in.timestamp = OS;
	check_pre(in, .action = KEEP, .direct = true, .timestamp = OS, .next_audio_ts_min = OS + DUR);
}

/* Direct means strictly less than MAX_TS_VAR from os_time, either way. */
static void pre_direct_at_max_ts_var(void **state)
{
	(void)state;
	const uint64_t offsets[] = {MAX_VAR - 1, MAX_VAR, MAX_VAR + 1};
	for (size_t i = 0; i < 3; i++) {
		bool direct = offsets[i] < MAX_VAR;
		uint64_t ahead = OS + offsets[i];
		uint64_t behind = OS - offsets[i];
		struct pre_in in = {.timestamp = ahead, .os_time = OS, .duration = DUR};
		if (direct)
			check_pre(in, .action = KEEP, .direct = true, .timestamp = ahead,
				  .next_audio_ts_min = ahead + DUR);
		else
			check_pre(in, .action = RESET, .timestamp = ahead, .next_audio_ts_min = ahead + DUR,
				  .timing_adjust = OS - ahead);
		in.timestamp = behind;
		if (direct)
			check_pre(in, .action = KEEP, .direct = true, .timestamp = behind,
				  .next_audio_ts_min = behind + DUR);
		else
			check_pre(in, .action = RESET, .timestamp = behind, .next_audio_ts_min = behind + DUR,
				  .timing_adjust = OS - behind);
	}
}

/* Timing not set: reset it to this packet, timing_adjust = os_time -
 * timestamp. The expected timestamp is not looked at, so a packet that
 * continues the previous one is not smoothed. */
static void pre_reset_when_timing_not_set(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = TS, .os_time = OS, .duration = DUR};
	check_pre(in, .action = RESET, .timestamp = TS, .next_audio_ts_min = TS + DUR, .timing_adjust = OS - TS);
	in.next_audio_ts_min = TS + 1;
	check_pre(in, .action = RESET, .timestamp = TS, .next_audio_ts_min = TS + DUR, .timing_adjust = OS - TS);
	in.next_audio_ts_min = TS + 10 * MAX_VAR;
	check_pre(in, .action = RESET, .timestamp = TS, .next_audio_ts_min = TS + DUR, .timing_adjust = OS - TS);
	/* an async source's flags change nothing here */
	in.async_unbuffered = true;
	in.async_decoupled = true;
	check_pre(in, .action = RESET, .timestamp = TS, .next_audio_ts_min = TS + DUR, .timing_adjust = OS - TS);
}

/* A timestamp ahead of the system clock: timing_adjust wraps to the
 * two's complement of the distance (characterized, not endorsed: it only
 * works because every later sum wraps too). */
static void pre_reset_ahead_of_clock_wraps(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = OS + 3 * MAX_VAR, .os_time = OS, .duration = DUR};
	check_pre(in, .action = RESET, .timestamp = OS + 3 * MAX_VAR, .next_audio_ts_min = OS + 3 * MAX_VAR + DUR,
		  .timing_adjust = UINT64_MAX - 3 * MAX_VAR + 1);
}

/* Timing set, no expected timestamp (next_audio_ts_min 0, before the first
 * packet or after audio was reset): keep the packet's timestamp. */
static void pre_keep_when_next_is_zero(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = TS, .os_time = OS, .duration = DUR, .timing_set = true};
	check_pre(in, .action = KEEP, .timestamp = TS, .next_audio_ts_min = TS + DUR);
	in.async_unbuffered = true;
	in.async_decoupled = true;
	check_pre(in, .action = KEEP, .timestamp = TS, .next_audio_ts_min = TS + DUR);
	/* a packet at 0 too */
	in.timestamp = 0;
	check_pre(in, .action = KEEP, .timestamp = 0, .next_audio_ts_min = DUR);
}

/* Within TS_SMOOTHING_THRESHOLD of the expected timestamp: the packet
 * continues the previous one and goes on the expected timestamp. The drift
 * error is expected - timestamp, positive when the samples run ahead. */
static void pre_smooth(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = TS, .os_time = OS, .duration = DUR, .timing_set = true};
	in.next_audio_ts_min = TS;
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = TS, .next_audio_ts_min = TS + DUR);
	in.next_audio_ts_min = TS + 3 * MS;
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = TS + 3 * MS,
		  .next_audio_ts_min = TS + 3 * MS + DUR, .drift_error = 3 * MS);
	in.next_audio_ts_min = TS - 3 * MS;
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = TS - 3 * MS,
		  .next_audio_ts_min = TS - 3 * MS + DUR, .drift_error = -(int64_t)(3 * MS));
	/* only one of the async flags: no re-anchor */
	in.async_unbuffered = true;
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = TS - 3 * MS,
		  .next_audio_ts_min = TS - 3 * MS + DUR, .drift_error = -(int64_t)(3 * MS));
	in.async_unbuffered = false;
	in.async_decoupled = true;
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = TS - 3 * MS,
		  .next_audio_ts_min = TS - 3 * MS + DUR, .drift_error = -(int64_t)(3 * MS));
}

/* TS_SMOOTHING_THRESHOLD is exclusive: 70 ms - 1 ns is smoothed, 70 ms and
 * 70 ms + 1 ns are logged and keep the packet's timestamp, either side. */
static void pre_at_smoothing_threshold(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = TS, .os_time = OS, .duration = DUR, .timing_set = true};

	in.next_audio_ts_min = TS + SMOOTH - 1;
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = TS + SMOOTH - 1,
		  .next_audio_ts_min = TS + SMOOTH - 1 + DUR, .drift_error = (int64_t)(SMOOTH - 1));
	in.next_audio_ts_min = TS - (SMOOTH - 1);
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = TS - (SMOOTH - 1),
		  .next_audio_ts_min = TS - (SMOOTH - 1) + DUR, .drift_error = -(int64_t)(SMOOTH - 1));

	in.next_audio_ts_min = TS + SMOOTH;
	check_pre(in, .action = EXCEEDED, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = SMOOTH);
	in.next_audio_ts_min = TS - SMOOTH;
	check_pre(in, .action = EXCEEDED, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = SMOOTH);

	in.next_audio_ts_min = TS + SMOOTH + 1;
	check_pre(in, .action = EXCEEDED, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = SMOOTH + 1);
	in.next_audio_ts_min = TS - SMOOTH - 1;
	check_pre(in, .action = EXCEEDED, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = SMOOTH + 1);
}

/* MAX_TS_VAR is exclusive the other way: up to 2 s off is logged, 2 s +
 * 1 ns is a jump. A jump resets the timing to the packet (handle_ts_jump()),
 * and the packet keeps its timestamp. */
static void pre_at_max_ts_var(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = TS, .os_time = OS, .duration = DUR, .timing_set = true};

	in.next_audio_ts_min = TS + MAX_VAR - 1;
	check_pre(in, .action = EXCEEDED, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = MAX_VAR - 1);
	in.next_audio_ts_min = TS + MAX_VAR;
	check_pre(in, .action = EXCEEDED, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = MAX_VAR);
	in.next_audio_ts_min = TS - MAX_VAR;
	check_pre(in, .action = EXCEEDED, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = MAX_VAR);

	in.next_audio_ts_min = TS + MAX_VAR + 1;
	check_pre(in, .action = JUMP, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = MAX_VAR + 1,
		  .timing_adjust = OS - TS);
	in.next_audio_ts_min = TS - MAX_VAR - 1;
	check_pre(in, .action = JUMP, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = MAX_VAR + 1,
		  .timing_adjust = OS - TS);
	/* far off, and the async flags change nothing */
	in.next_audio_ts_min = TS + 1000 * MAX_VAR;
	in.async_unbuffered = true;
	in.async_decoupled = true;
	check_pre(in, .action = JUMP, .timestamp = TS, .next_audio_ts_min = TS + DUR, .diff = 1000 * MAX_VAR,
		  .timing_adjust = OS - TS);
}

/* A direct timestamp never jumps: more than 2 s from the expected one, it
 * is logged as exceeding the smoothing threshold. Close to it, it is
 * smoothed like any other. */
static void pre_direct_never_jumps(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = OS, .os_time = OS, .duration = DUR, .timing_set = true};
	in.next_audio_ts_min = OS + 3 * MAX_VAR;
	check_pre(in, .action = EXCEEDED, .direct = true, .timestamp = OS, .next_audio_ts_min = OS + DUR,
		  .diff = 3 * MAX_VAR);
	in.next_audio_ts_min = OS + 2 * MS;
	check_pre(in, .action = SMOOTHED, .direct = true, .continuous = true, .timestamp = OS + 2 * MS,
		  .next_audio_ts_min = OS + 2 * MS + DUR, .drift_error = 2 * MS);
	/* timing_set false does not matter when direct */
	in.timing_set = false;
	check_pre(in, .action = SMOOTHED, .direct = true, .continuous = true, .timestamp = OS + 2 * MS,
		  .next_audio_ts_min = OS + 2 * MS + DUR, .drift_error = 2 * MS);
}

/* An unbuffered, decoupled async source re-anchors its timing on every
 * smoothed packet: timing_adjust = os_time - the packet's own timestamp
 * (not the smoothed one). */
static void pre_async_decoupled_reanchor(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = TS,
			    .os_time = OS,
			    .duration = DUR,
			    .timing_set = true,
			    .next_audio_ts_min = TS + 4 * MS,
			    .async_unbuffered = true,
			    .async_decoupled = true};
	check_pre(in, .action = SMOOTHED, .reanchor = true, .continuous = true, .timestamp = TS + 4 * MS,
		  .next_audio_ts_min = TS + 4 * MS + DUR, .timing_adjust = OS - TS, .drift_error = 4 * MS);
	/* with a direct timestamp the re-anchor follows the direct store */
	in.timestamp = OS - 1 * MS;
	in.next_audio_ts_min = OS;
	check_pre(in, .action = SMOOTHED, .direct = true, .reanchor = true, .continuous = true, .timestamp = OS,
		  .next_audio_ts_min = OS + DUR, .timing_adjust = 1 * MS, .drift_error = 1 * MS);
}

/* The arithmetic wraps, as uint64_t does (characterized, not endorsed):
 * the packet's end past UINT64_MAX, a duration of 0, and a smoothing across
 * the end of the range, which the distance does not see (it is not modular),
 * so it is a jump. */
static void pre_wraps(void **state)
{
	(void)state;
	struct pre_in in = {.timestamp = UINT64_MAX - 5, .os_time = OS, .duration = DUR, .timing_set = true};
	check_pre(in, .action = KEEP, .timestamp = UINT64_MAX - 5, .next_audio_ts_min = DUR - 6);
	in.duration = 0;
	check_pre(in, .action = KEEP, .timestamp = UINT64_MAX - 5, .next_audio_ts_min = UINT64_MAX - 5);
	in.duration = DUR;
	in.next_audio_ts_min = 3;
	check_pre(in, .action = JUMP, .timestamp = UINT64_MAX - 5, .next_audio_ts_min = DUR - 6, .diff = UINT64_MAX - 8,
		  .timing_adjust = OS + 6);
	/* the smoothed end wraps too */
	in.timestamp = UINT64_MAX - 2 * MS;
	in.next_audio_ts_min = UINT64_MAX - 1 * MS;
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = UINT64_MAX - 1 * MS,
		  .next_audio_ts_min = DUR - 1 * MS - 1, .drift_error = 1 * MS);
	/* the largest drift error each way */
	in.timestamp = TS;
	in.next_audio_ts_min = TS + SMOOTH - 1;
	check_pre(in, .action = SMOOTHED, .continuous = true, .timestamp = TS + SMOOTH - 1,
		  .next_audio_ts_min = TS + SMOOTH - 1 + DUR, .drift_error = (int64_t)SMOOTH - 1);
	/* direct at 0: os_time and timestamp both near 0 */
	in.timestamp = 0;
	in.os_time = 1;
	in.next_audio_ts_min = 0;
	check_pre(in, .action = KEEP, .direct = true, .timestamp = 0, .next_audio_ts_min = DUR);
}

/* ------------------------------------------------------------------------ */
/* obs_source_audio_timing_under_lock                                        */

struct under_in {
	uint64_t timestamp;
	uint64_t packet_timing_adjust;
	uint64_t data_timestamp;
	uint64_t os_time;
	uint64_t next_audio_ts_min;
	uint64_t resample_offset;
	int64_t sync_offset;
	uint64_t timing_adjust;
	uint64_t next_audio_sys_ts_min;
	int64_t last_sync_offset;
	bool continuous;
};

struct under_out {
	uint64_t timestamp;
	bool push_back;
	bool reset_timing;
	uint64_t timing_adjust;
	uint64_t next_audio_sys_ts_min;
	int64_t last_sync_offset;
};

static void check_under_at(struct under_in in, struct under_out e, int line)
{
	struct obs_source_audio_timing_under t;
	memset(&t, GARBAGE, sizeof(t));
	t.timestamp = in.timestamp;
	t.packet_timing_adjust = in.packet_timing_adjust;
	t.data_timestamp = in.data_timestamp;
	t.os_time = in.os_time;
	t.next_audio_ts_min = in.next_audio_ts_min;
	t.resample_offset = in.resample_offset;
	t.sync_offset = in.sync_offset;
	t.timing_adjust = in.timing_adjust;
	t.next_audio_sys_ts_min = in.next_audio_sys_ts_min;
	t.last_sync_offset = in.last_sync_offset;
	t.continuous = in.continuous;
	t.push_back = !e.push_back;
	t.reset_timing = !e.reset_timing;
	obs_source_audio_timing_under_lock(&t);

	check_u64(t.timestamp, e.timestamp, "timestamp", line);
	check_bool(t.push_back, e.push_back, "push_back", line);
	check_bool(t.reset_timing, e.reset_timing, "reset_timing", line);
	check_u64(t.timing_adjust, e.timing_adjust, "timing_adjust", line);
	check_u64(t.next_audio_sys_ts_min, e.next_audio_sys_ts_min, "next_audio_sys_ts_min", line);
	check_i64(t.last_sync_offset, e.last_sync_offset, "last_sync_offset", line);
	/* the inputs are left alone */
	check_u64(t.packet_timing_adjust, in.packet_timing_adjust, "packet_timing_adjust", line);
	check_u64(t.data_timestamp, in.data_timestamp, "data_timestamp", line);
	check_u64(t.os_time, in.os_time, "os_time", line);
	check_u64(t.next_audio_ts_min, in.next_audio_ts_min, "next_audio_ts_min", line);
	check_u64(t.resample_offset, in.resample_offset, "resample_offset", line);
	check_i64(t.sync_offset, in.sync_offset, "sync_offset", line);
	check_bool(t.continuous, in.continuous, "continuous", line);
}

#define check_under(in, ...) check_under_at(in, (struct under_out){__VA_ARGS__}, __LINE__)

/* A media source: timestamps from TS, timing_adjust = OS - TS. */
#define ADJ (OS - TS)

static struct under_in media_packet(uint64_t ts)
{
	return (struct under_in){.timestamp = ts,
				 .packet_timing_adjust = ADJ,
				 .data_timestamp = ts,
				 .os_time = OS,
				 .next_audio_ts_min = ts + DUR,
				 .timing_adjust = ADJ};
}

/* The packet starts where the previous one ended in system time: append
 * it. next_audio_sys_ts_min becomes its end. */
static void under_push_back_when_contiguous(void **state)
{
	(void)state;
	struct under_in in = media_packet(TS);
	in.next_audio_sys_ts_min = TS + ADJ;
	check_under(in, .timestamp = TS + ADJ, .push_back = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ);
	/* continuous or not */
	in.continuous = true;
	check_under(in, .timestamp = TS + ADJ, .push_back = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ);
}

/* No previous packet in system time (0): place it by its timestamp. */
static void under_place_when_next_sys_is_zero(void **state)
{
	(void)state;
	struct under_in in = media_packet(TS);
	check_under(in, .timestamp = TS + ADJ, .timing_adjust = ADJ, .next_audio_sys_ts_min = TS + DUR + ADJ);
	/* even far from anything, nothing is reset */
	in.timestamp = TS + 100 * MAX_VAR;
	in.next_audio_ts_min = in.timestamp + DUR;
	check_under(in, .timestamp = TS + 100 * MAX_VAR + ADJ, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + 100 * MAX_VAR + DUR + ADJ);
}

/* Within TS_SMOOTHING_THRESHOLD (exclusive) of next_audio_sys_ts_min,
 * either side: append. The packet keeps its own system time (the append
 * ignores it). From 70 ms up to 2 s: place it. */
static void under_at_smoothing_threshold(void **state)
{
	(void)state;
	struct under_in in = media_packet(TS);
	const uint64_t sys = TS + ADJ;

	in.next_audio_sys_ts_min = sys + SMOOTH - 1;
	check_under(in, .timestamp = sys, .push_back = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ);
	in.next_audio_sys_ts_min = sys - (SMOOTH - 1);
	check_under(in, .timestamp = sys, .push_back = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ);
	in.next_audio_sys_ts_min = sys + SMOOTH;
	check_under(in, .timestamp = sys, .timing_adjust = ADJ, .next_audio_sys_ts_min = TS + DUR + ADJ);
	in.next_audio_sys_ts_min = sys - SMOOTH;
	check_under(in, .timestamp = sys, .timing_adjust = ADJ, .next_audio_sys_ts_min = TS + DUR + ADJ);
	in.next_audio_sys_ts_min = sys + SMOOTH + 1;
	check_under(in, .timestamp = sys, .timing_adjust = ADJ, .next_audio_sys_ts_min = TS + DUR + ADJ);
}

/* More than MAX_TS_VAR (exclusive) from next_audio_sys_ts_min and not
 * continuous: reset the timing to the packet's own timestamp, so it is
 * placed at os_time. At exactly 2 s it is placed by its timestamp. */
static void under_reset_at_max_ts_var(void **state)
{
	(void)state;
	struct under_in in = media_packet(TS);
	const uint64_t sys = TS + ADJ;
	/* the timing moved since the last reset: the packet is 30 s off */
	in.packet_timing_adjust = ADJ + 30000 * MS;
	in.timing_adjust = ADJ + 30000 * MS;
	in.os_time = OS + 7 * MS;
	const uint64_t moved = sys + 30000 * MS;

	in.next_audio_sys_ts_min = moved + MAX_VAR;
	check_under(in, .timestamp = moved, .timing_adjust = ADJ + 30000 * MS,
		    .next_audio_sys_ts_min = TS + DUR + ADJ + 30000 * MS);
	in.next_audio_sys_ts_min = moved - MAX_VAR;
	check_under(in, .timestamp = moved, .timing_adjust = ADJ + 30000 * MS,
		    .next_audio_sys_ts_min = TS + DUR + ADJ + 30000 * MS);

	in.next_audio_sys_ts_min = moved + MAX_VAR + 1;
	check_under(in, .timestamp = OS + 7 * MS, .reset_timing = true, .timing_adjust = OS + 7 * MS - TS,
		    .next_audio_sys_ts_min = TS + DUR + OS + 7 * MS - TS);
	in.next_audio_sys_ts_min = moved - MAX_VAR - 1;
	check_under(in, .timestamp = OS + 7 * MS, .reset_timing = true, .timing_adjust = OS + 7 * MS - TS,
		    .next_audio_sys_ts_min = TS + DUR + OS + 7 * MS - TS);
}

/* #522: when the audio was continuous (smoothed onto the previous packet),
 * a system time more than 2 s away means the timing moved (a Video Delay
 * (Async) filter), not the audio: no reset, place it by the moved timing. */
static void under_no_reset_when_continuous(void **state)
{
	(void)state;
	struct under_in in = media_packet(TS);
	in.continuous = true;
	in.packet_timing_adjust = ADJ + 4000 * MS;
	in.timing_adjust = ADJ + 4000 * MS;
	in.next_audio_sys_ts_min = TS + ADJ;
	check_under(in, .timestamp = TS + ADJ + 4000 * MS, .timing_adjust = ADJ + 4000 * MS,
		    .next_audio_sys_ts_min = TS + DUR + ADJ + 4000 * MS);
	/* the reset uses the packet's own timestamp, not the smoothed one */
	in.continuous = false;
	in.timestamp = TS + 3 * MS;
	check_under(in, .timestamp = OS, .reset_timing = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ);
}

/* The sync offset is added and the resampler offset taken off after the
 * comparison: neither moves the push-back decision (41fa9c1bd). */
static void under_offsets_after_the_comparison(void **state)
{
	(void)state;
	struct under_in in = media_packet(TS);
	in.next_audio_sys_ts_min = TS + ADJ;
	in.sync_offset = 300 * MS;
	in.last_sync_offset = 300 * MS;
	in.resample_offset = 1 * MS;
	check_under(in, .timestamp = TS + ADJ + 299 * MS, .push_back = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ, .last_sync_offset = 300 * MS);
	in.sync_offset = -(int64_t)(300 * MS);
	in.last_sync_offset = -(int64_t)(300 * MS);
	check_under(in, .timestamp = TS + ADJ - 301 * MS, .push_back = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ, .last_sync_offset = -(int64_t)(300 * MS));
	/* on a reset too: os_time + the offsets */
	in.next_audio_sys_ts_min = TS + ADJ + 3 * MAX_VAR;
	check_under(in, .timestamp = OS - 301 * MS, .reset_timing = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ, .last_sync_offset = -(int64_t)(300 * MS));
}

/* next_audio_sys_ts_min is next_audio_ts_min plus the timing_adjust read
 * under the lock, which the video thread may have moved since the packet's
 * was read; the packet itself uses the one read before the lock. */
static void under_next_sys_uses_timing_adjust_under_lock(void **state)
{
	(void)state;
	struct under_in in = media_packet(TS);
	in.next_audio_sys_ts_min = TS + ADJ;
	in.timing_adjust = ADJ + 33 * MS;
	check_under(in, .timestamp = TS + ADJ, .push_back = true, .timing_adjust = ADJ + 33 * MS,
		    .next_audio_sys_ts_min = TS + DUR + ADJ + 33 * MS);
	/* after a reset, the reset's */
	in.next_audio_sys_ts_min = TS + ADJ + 3 * MAX_VAR;
	in.os_time = OS + 1;
	check_under(in, .timestamp = OS + 1, .reset_timing = true, .timing_adjust = ADJ + 1,
		    .next_audio_sys_ts_min = TS + DUR + ADJ + 1);
}

/* #333: after any change of the sync offset, the packet is placed by its
 * timestamp, even when it would be appended, and the new offset is kept.
 * From 0 too (f7c446e0b). */
static void under_sync_offset_changed_places(void **state)
{
	(void)state;
	struct under_in in = media_packet(TS);
	in.next_audio_sys_ts_min = TS + ADJ;
	in.sync_offset = 300 * MS;
	check_under(in, .timestamp = TS + ADJ + 300 * MS, .timing_adjust = ADJ, .next_audio_sys_ts_min = TS + DUR + ADJ,
		    .last_sync_offset = 300 * MS);
	in.last_sync_offset = 300 * MS;
	in.sync_offset = 100 * MS;
	check_under(in, .timestamp = TS + ADJ + 100 * MS, .timing_adjust = ADJ, .next_audio_sys_ts_min = TS + DUR + ADJ,
		    .last_sync_offset = 100 * MS);
	/* back to 0 */
	in.last_sync_offset = 100 * MS;
	in.sync_offset = 0;
	check_under(in, .timestamp = TS + ADJ, .timing_adjust = ADJ, .next_audio_sys_ts_min = TS + DUR + ADJ);
	/* with a reset as well */
	in.next_audio_sys_ts_min = TS + ADJ + 3 * MAX_VAR;
	in.last_sync_offset = 1;
	check_under(in, .timestamp = OS, .reset_timing = true, .timing_adjust = ADJ,
		    .next_audio_sys_ts_min = TS + DUR + ADJ);
}

/* Characterized, not endorsed: a negative sync offset larger than the
 * packet's system time wraps to near UINT64_MAX, and so does a resampler
 * offset larger than it. libobs then places the packet as if it were
 * centuries late. */
static void under_negative_sync_offset_wraps(void **state)
{
	(void)state;
	struct under_in in = {.timestamp = 1 * MS,
			      .data_timestamp = 1 * MS,
			      .os_time = 2 * MS,
			      .next_audio_ts_min = 1 * MS + DUR,
			      .sync_offset = -(int64_t)(5 * MS),
			      .last_sync_offset = -(int64_t)(5 * MS)};
	check_under(in, .timestamp = UINT64_MAX - 4 * MS + 1, .next_audio_sys_ts_min = 1 * MS + DUR,
		    .last_sync_offset = -(int64_t)(5 * MS));
	in.sync_offset = INT64_MIN;
	in.last_sync_offset = INT64_MIN;
	check_under(in, .timestamp = 1 * MS + 0x8000000000000000ULL, .next_audio_sys_ts_min = 1 * MS + DUR,
		    .last_sync_offset = INT64_MIN);
	in.sync_offset = 0;
	in.last_sync_offset = 0;
	in.resample_offset = 2 * MS;
	check_under(in, .timestamp = UINT64_MAX - 1 * MS + 1, .next_audio_sys_ts_min = 1 * MS + DUR);
}

/* Characterized, not endorsed: timestamp + timing_adjust past UINT64_MAX
 * wraps (a media timestamp far ahead of the system clock gives a
 * timing_adjust near UINT64_MAX), and so do the sync offset and
 * next_audio_sys_ts_min; the sums come back to the system time. */
static void under_timestamp_plus_adjust_wraps(void **state)
{
	(void)state;
	const uint64_t ts = OS + 10 * MAX_VAR;
	const uint64_t adj = OS - ts; /* wraps */
	struct under_in in = {.timestamp = ts,
			      .packet_timing_adjust = adj,
			      .data_timestamp = ts,
			      .os_time = OS,
			      .next_audio_ts_min = ts + DUR,
			      .timing_adjust = adj,
			      .next_audio_sys_ts_min = OS};
	check_under(in, .timestamp = OS, .push_back = true, .timing_adjust = adj, .next_audio_sys_ts_min = OS + DUR);
	in.timestamp = UINT64_MAX;
	in.packet_timing_adjust = 2;
	in.next_audio_sys_ts_min = 1;
	check_under(in, .timestamp = 1, .push_back = true, .timing_adjust = adj, .next_audio_sys_ts_min = OS + DUR);
	in.sync_offset = INT64_MAX;
	in.last_sync_offset = INT64_MAX;
	check_under(in, .timestamp = 0x8000000000000000ULL, .push_back = true, .timing_adjust = adj,
		    .next_audio_sys_ts_min = OS + DUR, .last_sync_offset = INT64_MAX);
	/* the distance is not modular: 1 and UINT64_MAX are far apart */
	in.sync_offset = 0;
	in.last_sync_offset = 0;
	in.packet_timing_adjust = 0;
	check_under(in, .timestamp = OS, .reset_timing = true, .timing_adjust = adj, .next_audio_sys_ts_min = OS + DUR);
}

/* ------------------------------------------------------------------------ */
/* obs_source_audio_timing_buffer_start                                      */

/* #286: an emptied buffer starts at os_time + the sync offset - the
 * resampler offset; at 0 (audio reset with no time) it stays 0. */
static void buffer_start(void **state)
{
	(void)state;
	check_u64(obs_source_audio_timing_buffer_start(OS, 0, 0), OS, "start", __LINE__);
	check_u64(obs_source_audio_timing_buffer_start(OS, 300 * MS, 0), OS + 300 * MS, "start", __LINE__);
	check_u64(obs_source_audio_timing_buffer_start(OS, -(int64_t)(300 * MS), 1 * MS), OS - 301 * MS, "start",
		  __LINE__);
	check_u64(obs_source_audio_timing_buffer_start(0, 300 * MS, 1 * MS), 0, "start", __LINE__);
	check_u64(obs_source_audio_timing_buffer_start(0, INT64_MIN, UINT64_MAX), 0, "start", __LINE__);
	/* characterized, not endorsed: it wraps */
	check_u64(obs_source_audio_timing_buffer_start(1, -2, 0), UINT64_MAX, "start", __LINE__);
	check_u64(obs_source_audio_timing_buffer_start(1, 0, 2), UINT64_MAX, "start", __LINE__);
	check_u64(obs_source_audio_timing_buffer_start(UINT64_MAX, INT64_MAX, 0), INT64_MAX - 1ULL, "start", __LINE__);
}

/* ------------------------------------------------------------------------ */
/* Packet sequences through a fake source                                    */

/* The fields of struct obs_source the steps read and write, and a model of
 * the rest of source_output_audio_data() (as of the move). */
struct fake_source {
	bool timing_set;
	uint64_t timing_adjust;
	uint64_t next_audio_ts_min;
	uint64_t next_audio_sys_ts_min;
	uint64_t resample_offset;
	int64_t sync_offset;
	int64_t last_sync_offset;
	bool async_unbuffered;
	bool async_decoupled;
	/* the buffer: where it starts and where its data ends, in ns */
	uint64_t audio_ts;
	uint64_t buf_end;
	/* the last packet */
	int action;
	bool pushed;
	uint64_t placed;
	int64_t drift_error;
};

/* reset_audio_data(): empty the buffer for packets at os_time */
static void fake_reset_audio_data(struct fake_source *f, uint64_t os_time)
{
	f->audio_ts = os_time;
	f->buf_end = os_time;
	f->next_audio_sys_ts_min = os_time;
}

/* One packet of `duration` ns at `ts`, at system time `os_time`. */
static void fake_packet(struct fake_source *f, uint64_t ts, uint64_t os_time, uint64_t duration)
{
	struct obs_source_audio_timing_pre pre = {
		.timestamp = ts,
		.os_time = os_time,
		.duration = duration,
		.next_audio_ts_min = f->next_audio_ts_min,
		.timing_set = f->timing_set,
		.async_unbuffered = f->async_unbuffered,
		.async_decoupled = f->async_decoupled,
	};
	obs_source_audio_timing_pre_lock(&pre);
	if (pre.direct) {
		f->timing_adjust = 0;
		f->timing_set = true;
	}
	switch (pre.action) {
	case RESET:
		f->timing_set = true;
		f->timing_adjust = pre.timing_adjust;
		break;
	case JUMP:
		/* handle_ts_jump(): reset_audio_timing(), then
		 * reset_audio_data_at_sys_time() */
		f->timing_set = true;
		f->timing_adjust = os_time - ts;
		fake_reset_audio_data(f, os_time);
		f->audio_ts = obs_source_audio_timing_buffer_start(os_time, f->sync_offset, f->resample_offset);
		f->buf_end = f->audio_ts;
		break;
	case SMOOTHED:
		if (pre.reanchor)
			f->timing_adjust = pre.timing_adjust;
		break;
	case KEEP:
	case EXCEEDED:
		break;
	}
	f->action = (int)pre.action;
	f->drift_error = pre.drift_error;
	f->next_audio_ts_min = pre.next_audio_ts_min;

	struct obs_source_audio_timing_under under = {
		.timestamp = pre.timestamp,
		.packet_timing_adjust = f->timing_adjust,
		.data_timestamp = ts,
		.os_time = os_time,
		.next_audio_ts_min = f->next_audio_ts_min,
		.resample_offset = f->resample_offset,
		.sync_offset = f->sync_offset,
		.timing_adjust = f->timing_adjust,
		.next_audio_sys_ts_min = f->next_audio_sys_ts_min,
		.last_sync_offset = f->last_sync_offset,
		.continuous = pre.continuous,
	};
	obs_source_audio_timing_under_lock(&under);
	if (under.reset_timing) {
		f->timing_set = true;
		f->timing_adjust = under.timing_adjust;
	}
	f->next_audio_sys_ts_min = under.next_audio_sys_ts_min;
	f->last_sync_offset = under.last_sync_offset;

	f->pushed = under.push_back && f->audio_ts;
	if (f->pushed) {
		/* source_output_audio_push_back() */
		f->placed = f->buf_end;
	} else {
		/* source_output_audio_place() */
		if (!f->audio_ts || under.timestamp < f->audio_ts)
			fake_reset_audio_data(f, under.timestamp);
		f->placed = under.timestamp;
	}
	f->buf_end = f->placed + duration;
}

static void check_packet_at(const struct fake_source *f, int action, bool pushed, uint64_t placed, int line)
{
	if (f->action != action) {
		fprintf(stderr, "line %d: action is %s, expected %s\n", line, action_name(f->action),
			action_name(action));
		fail();
	}
	check_bool(f->pushed, pushed, "pushed", line);
	check_u64(f->placed, placed, "placed", line);
}

#define check_packet(f, action, pushed, placed) check_packet_at(f, action, pushed, placed, __LINE__)

/* A media source's packets, each where the previous one ended: the first
 * resets the timing and is placed, the rest are smoothed and appended. */
static void seq_contiguous_packets(void **state)
{
	(void)state;
	struct fake_source f = {0};
	fake_packet(&f, TS, OS, DUR);
	check_packet(&f, RESET, false, OS);
	check_u64(f.timing_adjust, ADJ, "timing_adjust", __LINE__);
	for (uint64_t i = 1; i < 5; i++) {
		fake_packet(&f, TS + i * DUR, OS + i * DUR + 1 * MS, DUR);
		check_packet(&f, SMOOTHED, true, OS + i * DUR);
		check_i64(f.drift_error, 0, "drift_error", __LINE__);
	}
	check_u64(f.next_audio_sys_ts_min, OS + 5 * DUR, "next_audio_sys_ts_min", __LINE__);
	/* jitter within 70 ms is smoothed away */
	fake_packet(&f, TS + 5 * DUR + 50 * MS, OS + 5 * DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + 5 * DUR);
	check_i64(f.drift_error, -(int64_t)(50 * MS), "drift_error", __LINE__);
}

/* A system-clock source (Pulse, PipeWire): direct timestamps, no
 * timing_adjust. */
static void seq_direct_timestamps(void **state)
{
	(void)state;
	struct fake_source f = {0};
	fake_packet(&f, OS - 10 * MS, OS, DUR);
	check_packet(&f, KEEP, false, OS - 10 * MS);
	check_bool(f.timing_set, true, "timing_set", __LINE__);
	check_u64(f.timing_adjust, 0, "timing_adjust", __LINE__);
	fake_packet(&f, OS - 10 * MS + DUR, OS + DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS - 10 * MS + DUR);
	/* a gap of 100 ms: logged, placed by its timestamp after the gap */
	fake_packet(&f, OS - 10 * MS + 2 * DUR + 100 * MS, OS + 2 * DUR + 100 * MS, DUR);
	check_packet(&f, EXCEEDED, false, OS - 10 * MS + 2 * DUR + 100 * MS);
}

/* #333: a sync offset changed between packets places the next packet by
 * its timestamp with the new offset; the one after is appended again. */
static void seq_sync_offset_changed(void **state)
{
	(void)state;
	struct fake_source f = {0};
	fake_packet(&f, TS, OS, DUR);
	fake_packet(&f, TS + DUR, OS + DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + DUR);

	f.sync_offset = 300 * MS;
	fake_packet(&f, TS + 2 * DUR, OS + 2 * DUR, DUR);
	check_packet(&f, SMOOTHED, false, OS + 2 * DUR + 300 * MS);
	check_i64(f.last_sync_offset, 300 * MS, "last_sync_offset", __LINE__);
	fake_packet(&f, TS + 3 * DUR, OS + 3 * DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + 3 * DUR + 300 * MS);

	/* lowered: placed earlier than the buffer's end */
	f.sync_offset = 100 * MS;
	fake_packet(&f, TS + 4 * DUR, OS + 4 * DUR, DUR);
	check_packet(&f, SMOOTHED, false, OS + 4 * DUR + 100 * MS);
	/* negative, before the buffer's start: source_output_audio_place()
	 * empties the buffer and sets next_audio_sys_ts_min to the packet's
	 * time, offset included, so the next packet is placed too */
	f.sync_offset = -(int64_t)(200 * MS);
	fake_packet(&f, TS + 5 * DUR, OS + 5 * DUR, DUR);
	check_packet(&f, SMOOTHED, false, OS + 5 * DUR - 200 * MS);
	check_u64(f.next_audio_sys_ts_min, OS + 5 * DUR - 200 * MS, "next_audio_sys_ts_min", __LINE__);
	fake_packet(&f, TS + 6 * DUR, OS + 6 * DUR, DUR);
	check_packet(&f, SMOOTHED, false, OS + 6 * DUR - 200 * MS);
	fake_packet(&f, TS + 7 * DUR, OS + 7 * DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + 7 * DUR - 200 * MS);
}

/* #286: after a timestamp jump the buffer starts at the reset time plus
 * the sync offset, and the packet, now continuing at the reset time, is
 * appended there. */
static void seq_jump_keeps_sync_offset(void **state)
{
	(void)state;
	struct fake_source f = {.sync_offset = 300 * MS, .last_sync_offset = 300 * MS};
	fake_packet(&f, TS, OS, DUR);
	check_packet(&f, RESET, false, OS + 300 * MS);
	/* the first placement set next_audio_sys_ts_min to the packet's time,
	 * offset included, so the second packet is placed by its timestamp
	 * too, right after the first */
	fake_packet(&f, TS + DUR, OS + DUR, DUR);
	check_packet(&f, SMOOTHED, false, OS + DUR + 300 * MS);
	fake_packet(&f, TS + 2 * DUR, OS + 2 * DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + 2 * DUR + 300 * MS);
	/* the file loops back to its start */
	fake_packet(&f, 0, OS + 3 * DUR, DUR);
	check_packet(&f, JUMP, true, OS + 3 * DUR + 300 * MS);
	check_u64(f.timing_adjust, OS + 3 * DUR, "timing_adjust", __LINE__);
	check_u64(f.audio_ts, OS + 3 * DUR + 300 * MS, "audio_ts", __LINE__);
	fake_packet(&f, DUR, OS + 4 * DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + 4 * DUR + 300 * MS);
}

/* #522: with a Video Delay (Async) filter the shown frame's timing moves
 * the source's timing_adjust by more than 2 s between two packets. The
 * audio is continuous, so it follows the timing instead of resetting. */
static void seq_async_delay_follows_timing(void **state)
{
	(void)state;
	struct fake_source f = {0};
	fake_packet(&f, TS, OS, DUR);
	fake_packet(&f, TS + DUR, OS + DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + DUR);
	/* the video thread moves the timing by 4 s */
	f.timing_adjust += 4000 * MS;
	fake_packet(&f, TS + 2 * DUR, OS + 2 * DUR, DUR);
	check_packet(&f, SMOOTHED, false, OS + 2 * DUR + 4000 * MS);
	check_bool(f.timing_set, true, "timing_set", __LINE__);
	check_u64(f.timing_adjust, ADJ + 4000 * MS, "timing_adjust", __LINE__);
	fake_packet(&f, TS + 3 * DUR, OS + 3 * DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + 3 * DUR + 4000 * MS);
	/* a packet that is not continuous (a gap over 70 ms) with its system
	 * time over 2 s off resets to the system clock */
	f.timing_adjust -= 4000 * MS;
	fake_packet(&f, TS + 4 * DUR + 100 * MS, OS + 4 * DUR, DUR);
	check_packet(&f, EXCEEDED, false, OS + 4 * DUR);
	check_u64(f.timing_adjust, OS + 4 * DUR - (TS + 4 * DUR + 100 * MS), "timing_adjust", __LINE__);
}

/* An unbuffered, decoupled async source: each smoothed packet re-anchors
 * timing_adjust on the system clock, so its packets follow os_time. */
static void seq_async_decoupled_reanchor(void **state)
{
	(void)state;
	struct fake_source f = {.async_unbuffered = true, .async_decoupled = true};
	fake_packet(&f, TS, OS, DUR);
	check_packet(&f, RESET, false, OS);
	/* the packet comes 5 ms late in system time */
	fake_packet(&f, TS + DUR, OS + DUR + 5 * MS, DUR);
	check_packet(&f, SMOOTHED, true, OS + DUR);
	check_u64(f.timing_adjust, ADJ + 5 * MS, "timing_adjust", __LINE__);
	check_u64(f.next_audio_sys_ts_min, OS + 2 * DUR + 5 * MS, "next_audio_sys_ts_min", __LINE__);
	fake_packet(&f, TS + 2 * DUR, OS + 2 * DUR + 5 * MS, DUR);
	check_packet(&f, SMOOTHED, true, OS + 2 * DUR);
}

/* next_audio_ts_min == 0 with the timing set (after the audio mix
 * restarted the source's audio, which only clears timing_set, or a source
 * whose first packet was direct... ): keep, then smooth. */
static void seq_next_audio_ts_min_zero(void **state)
{
	(void)state;
	struct fake_source f = {.timing_set = true, .timing_adjust = ADJ};
	fake_packet(&f, TS, OS, DUR);
	check_packet(&f, KEEP, false, OS);
	fake_packet(&f, TS + DUR, OS + DUR, DUR);
	check_packet(&f, SMOOTHED, true, OS + DUR);
	/* the audio mix resets the source: only timing_set. The reset puts
	 * the packet 21 ms after the buffer's end in system time, within
	 * TS_SMOOTHING_THRESHOLD, so it is appended */
	f.timing_set = false;
	fake_packet(&f, TS + 2 * DUR, OS + 3 * DUR, DUR);
	check_packet(&f, RESET, true, OS + 2 * DUR);
	check_u64(f.timing_adjust, OS + DUR - TS, "timing_adjust", __LINE__);
	/* 100 ms after it: placed by the new timing */
	f.timing_set = false;
	fake_packet(&f, TS + 3 * DUR, OS + 5 * DUR + 100 * MS, DUR);
	check_packet(&f, RESET, false, OS + 5 * DUR + 100 * MS);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(pre_direct_timestamps),
		cmocka_unit_test(pre_direct_at_max_ts_var),
		cmocka_unit_test(pre_reset_when_timing_not_set),
		cmocka_unit_test(pre_reset_ahead_of_clock_wraps),
		cmocka_unit_test(pre_keep_when_next_is_zero),
		cmocka_unit_test(pre_smooth),
		cmocka_unit_test(pre_at_smoothing_threshold),
		cmocka_unit_test(pre_at_max_ts_var),
		cmocka_unit_test(pre_direct_never_jumps),
		cmocka_unit_test(pre_async_decoupled_reanchor),
		cmocka_unit_test(pre_wraps),
		cmocka_unit_test(under_push_back_when_contiguous),
		cmocka_unit_test(under_place_when_next_sys_is_zero),
		cmocka_unit_test(under_at_smoothing_threshold),
		cmocka_unit_test(under_reset_at_max_ts_var),
		cmocka_unit_test(under_no_reset_when_continuous),
		cmocka_unit_test(under_offsets_after_the_comparison),
		cmocka_unit_test(under_next_sys_uses_timing_adjust_under_lock),
		cmocka_unit_test(under_sync_offset_changed_places),
		cmocka_unit_test(under_negative_sync_offset_wraps),
		cmocka_unit_test(under_timestamp_plus_adjust_wraps),
		cmocka_unit_test(buffer_start),
		cmocka_unit_test(seq_contiguous_packets),
		cmocka_unit_test(seq_direct_timestamps),
		cmocka_unit_test(seq_sync_offset_changed),
		cmocka_unit_test(seq_jump_keeps_sync_offset),
		cmocka_unit_test(seq_async_delay_follows_timing),
		cmocka_unit_test(seq_async_decoupled_reanchor),
		cmocka_unit_test(seq_next_audio_ts_min_zero),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
