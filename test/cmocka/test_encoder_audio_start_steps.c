#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-audio-start.h"
#include "util/util_uint64.h"

/*
 * Characterization of the encoder audio start alignment and the setter gates
 * of libobs/obs-encoder.c, moved to libobs/obs-encoder-audio-start.c before
 * their Rust port:
 * - receive_video(): the group start gate (obs_encoder_audio_start_group_ready),
 *   the wait for each paired audio encoder (_paired_late) and the pts step
 *   after an encode (_video_pts);
 * - buffer_audio(): the packet size (_size), the paired and unpaired branches
 *   (_paired, _unpaired), "no video yet" (_no_video) and the plan once the
 *   video has started (_packet: wait, truncate, clear, start from the buffer);
 * - calc_offset_size() (_offset_ns, _offset_size), start_from_buffer()
 *   (_buffer_offset) and push_back_audio() (_push_size);
 * - receive_audio()'s send loop (_can_send) and send_audio_data()'s pts step
 *   (_audio_pts);
 * - the first refusal of obs_encoder_set_gpu_scale_type() and
 *   obs_encoder_set_frame_rate_divisor() (_not_video) and the divisor rule
 *   (_divisor_ok).
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * buffer_audio(), start_from_buffer(), calc_offset_size() and
 * push_back_audio() are also copied below onto a stand-in encoder that counts
 * the bytes buffered, so the start of the audio is pinned end to end against
 * a video start: before, at and after it by one sample and by one packet, a
 * packet that ends exactly at it, the unpaired start, and buffered audio
 * below, at and above the offset, at blocksizes 4 and 8 (mono and stereo
 * float). The two conversions between nanoseconds and frames are the real
 * util_mul_div64(), as in obs-encoder.c.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. The real-libobs
 * tests (test_output_interleave_e2e, test_output_interleave_restart,
 * test_output_delay_e2e, test_encoder_error_stop, test_encoding_lag_rescale,
 * test_encoder_video_reset, test_multitrack_encoder_override) cover the
 * callers.
 *
 * Nothing here comes from outside OBS: timestamps come from the audio and
 * video threads, sizes and rates from libobs and the encoder plugins, the
 * type from a plugin and the divisor from the frontend.
 */

#define NS 1000000000ULL

static const uint64_t edge_u64[] = {0, 1, 2, 0x7fffffffffffffffULL, 0x8000000000000000ULL, UINT64_MAX - 1, UINT64_MAX};
#define N_EDGE_U64 (sizeof(edge_u64) / sizeof(edge_u64[0]))

/* ------------------------------------------------------------------------ */
/* receive_video() */

static void group_ready(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_U64; i++) {
		for (size_t j = 0; j < N_EDGE_U64; j++) {
			bool want = edge_u64[i] == edge_u64[j];
			assert_int_equal(obs_encoder_audio_start_group_ready(edge_u64[i], edge_u64[j]), want);
		}
	}
	assert_true(obs_encoder_audio_start_group_ready(1000000000, 1000000000));
	assert_false(obs_encoder_audio_start_group_ready(1000000000, 1000000001));
	assert_false(obs_encoder_audio_start_group_ready(1000000001, 1000000000));
	/* a group that has not started has start_timestamp 0: only a frame at
	 * 0 would match */
	assert_false(obs_encoder_audio_start_group_ready(0, 16666667));
}

/* The video waits while a paired audio encoder's first audio is after the
 * frame; audio at or before the frame lets it go. */
static void paired_late(void **state)
{
	(void)state;
	const uint64_t ts = 5000000000ULL;
	assert_true(obs_encoder_audio_start_paired_late(ts + 1, ts));
	assert_true(obs_encoder_audio_start_paired_late(ts + 21333333, ts));
	assert_false(obs_encoder_audio_start_paired_late(ts, ts));
	assert_false(obs_encoder_audio_start_paired_late(ts - 1, ts));
	assert_false(obs_encoder_audio_start_paired_late(0, ts));
	for (size_t i = 0; i < N_EDGE_U64; i++) {
		for (size_t j = 0; j < N_EDGE_U64; j++) {
			bool want = edge_u64[i] > edge_u64[j];
			assert_int_equal(obs_encoder_audio_start_paired_late(edge_u64[i], edge_u64[j]), want);
		}
	}
}

static void video_pts(void **state)
{
	(void)state;
	assert_int_equal(obs_encoder_audio_start_video_pts(0, 1, 1), 1);
	assert_int_equal(obs_encoder_audio_start_video_pts(0, 1001, 1), 1001);
	assert_int_equal(obs_encoder_audio_start_video_pts(30, 1, 2), 32);
	assert_int_equal(obs_encoder_audio_start_video_pts(1001 * 7, 1001, 3), 1001 * 10);
	assert_int_equal(obs_encoder_audio_start_video_pts(-5, 1, 1), -4);
	assert_int_equal(obs_encoder_audio_start_video_pts(-5, 0, 7), -5);
	assert_int_equal(obs_encoder_audio_start_video_pts(12, 7, 0), 12);
	/* timebase_num * frame_rate_divisor is a uint32_t product: it wraps
	 * before it is added (characterized, not endorsed; real timebases and
	 * divisors are small). */
	assert_int_equal(obs_encoder_audio_start_video_pts(100, 0x10000, 0x10000), 100);
	assert_int_equal(obs_encoder_audio_start_video_pts(100, 0x10000, 0x10001), 100 + 0x10000);
	assert_int_equal(obs_encoder_audio_start_video_pts(0, UINT32_MAX, UINT32_MAX), 1);
	assert_int_equal(obs_encoder_audio_start_video_pts(0, UINT32_MAX, 1), (int64_t)UINT32_MAX);
	assert_int_equal(obs_encoder_audio_start_video_pts(INT64_MAX - (int64_t)UINT32_MAX, UINT32_MAX, 1), INT64_MAX);
}

/* ------------------------------------------------------------------------ */
/* buffer_audio() and its helpers */

static void packet_size(void **state)
{
	(void)state;
	assert_int_equal(obs_encoder_audio_start_size(1024, 4), 4096);
	assert_int_equal(obs_encoder_audio_start_size(1024, 8), 8192);
	assert_int_equal(obs_encoder_audio_start_size(960, 4), 3840);
	assert_int_equal(obs_encoder_audio_start_size(0, 8), 0);
	assert_int_equal(obs_encoder_audio_start_size(1024, 0), 0);
	assert_int_equal(obs_encoder_audio_start_size(1, 4), 4);
#if SIZE_MAX > UINT32_MAX
	/* a size_t product: no 32-bit wrap */
	assert_true(obs_encoder_audio_start_size(UINT32_MAX, 8) == (size_t)UINT32_MAX * 8);
	/* and a size_t wrap past it (characterized, not endorsed) */
	assert_true(obs_encoder_audio_start_size(2, SIZE_MAX) == SIZE_MAX - 1);
#endif
}

static void branches(void **state)
{
	(void)state;
	/* not started, paired: wait for or start with the video */
	assert_true(obs_encoder_audio_start_paired(0, true));
	assert_false(obs_encoder_audio_start_unpaired(0, true));
	/* not started, unpaired: start at this packet */
	assert_false(obs_encoder_audio_start_paired(0, false));
	assert_true(obs_encoder_audio_start_unpaired(0, false));
	/* started: neither */
	for (size_t i = 1; i < N_EDGE_U64; i++) {
		assert_false(obs_encoder_audio_start_paired(edge_u64[i], true));
		assert_false(obs_encoder_audio_start_paired(edge_u64[i], false));
		assert_false(obs_encoder_audio_start_unpaired(edge_u64[i], true));
		assert_false(obs_encoder_audio_start_unpaired(edge_u64[i], false));
	}
}

static void no_video(void **state)
{
	(void)state;
	assert_true(obs_encoder_audio_start_no_video(0));
	for (size_t i = 1; i < N_EDGE_U64; i++)
		assert_false(obs_encoder_audio_start_no_video(edge_u64[i]));
}

static void expect_plan(uint64_t ts, uint64_t dur, uint64_t v, bool wait, bool truncate, bool clear, bool from_buffer)
{
	struct obs_encoder_audio_start_plan p = obs_encoder_audio_start_packet(ts, dur, v);
	if (p.wait != wait || p.truncate != truncate || p.clear != clear || p.from_buffer != from_buffer) {
		fprintf(stderr,
			"packet(%llu, %llu, %llu): wait %d truncate %d clear %d from_buffer %d, expected %d %d %d %d\n",
			(unsigned long long)ts, (unsigned long long)dur, (unsigned long long)v, p.wait, p.truncate,
			p.clear, p.from_buffer, wait, truncate, clear, from_buffer);
		fail();
	}
}

/* 1024 frames at 48 kHz: 21333333 ns. */
#define PKT 21333333ULL
#define V 10000000000ULL

static void packet_plan(void **state)
{
	(void)state;
	/* ends before the video starts: wait */
	expect_plan(V - 2 * PKT, PKT, V, true, false, false, false);
	/* ends exactly at the video's start: still wait */
	expect_plan(V - PKT, PKT, V, true, false, false, false);
	/* ends one ns into the video: start, truncated and cleared */
	expect_plan(V - PKT + 1, PKT, V, false, true, true, false);
	/* starts one ns before the video */
	expect_plan(V - 1, PKT, V, false, true, true, false);
	/* starts at the video: cleared, nothing truncated */
	expect_plan(V, PKT, V, false, false, true, false);
	/* starts one ns or one packet after: start from the buffer */
	expect_plan(V + 1, PKT, V, false, false, false, true);
	expect_plan(V + PKT, PKT, V, false, false, false, true);
	/* a packet of 0 frames: wait at or before the video, else from the
	 * buffer */
	expect_plan(V, 0, V, true, false, false, false);
	expect_plan(V + 1, 0, V, false, false, false, true);
	/* the end is a uint64_t sum: a packet whose end wraps past UINT64_MAX
	 * waits (characterized, not endorsed; timestamps are nanoseconds of
	 * os_gettime_ns()) */
	expect_plan(UINT64_MAX - 10, PKT, V, true, false, false, false);
	expect_plan(UINT64_MAX - PKT, PKT, UINT64_MAX - 1, false, true, true, false);
	expect_plan(UINT64_MAX - PKT, PKT, UINT64_MAX, true, false, false, false);
	/* a video start of 0 never gets here (no_video); with it, any packet
	 * not ending at 0 starts from the buffer */
	expect_plan(1, 0, 0, false, false, false, true);
}

static void offsets(void **state)
{
	(void)state;
	assert_true(obs_encoder_audio_start_offset_ns(V, V - 20834) == 20834);
	assert_true(obs_encoder_audio_start_offset_ns(V, 0) == V);
	assert_true(obs_encoder_audio_start_offset_ns(V, V) == 0);
	/* a uint64_t difference: wraps when the audio is after the video, which
	 * the callers never ask (characterized, not endorsed) */
	assert_true(obs_encoder_audio_start_offset_ns(V, V + 1) == UINT64_MAX);

	assert_int_equal(obs_encoder_audio_start_offset_size(1, 4), 4);
	assert_int_equal(obs_encoder_audio_start_offset_size(1024, 8), 8192);
	assert_int_equal(obs_encoder_audio_start_offset_size(0, 8), 0);
	assert_int_equal(obs_encoder_audio_start_offset_size(7, 0), 0);
#if SIZE_MAX > UINT32_MAX
	assert_true(obs_encoder_audio_start_offset_size(0x100000000ULL, 4) == 0x400000000ULL);
	assert_true(obs_encoder_audio_start_offset_size(UINT64_MAX, 1) == SIZE_MAX);
	/* a size_t product: wraps (characterized, not endorsed) */
	assert_true(obs_encoder_audio_start_offset_size(0x4000000000000001ULL, 4) == 4);
#endif

	assert_true(obs_encoder_audio_start_buffer_offset(V - 1, V));
	assert_false(obs_encoder_audio_start_buffer_offset(V, V));
	assert_false(obs_encoder_audio_start_buffer_offset(V + 1, V));
	assert_true(obs_encoder_audio_start_buffer_offset(0, 1));
	assert_false(obs_encoder_audio_start_buffer_offset(UINT64_MAX, UINT64_MAX));
}

static void push_size(void **state)
{
	(void)state;
	/* the offset below, at and above the packet */
	assert_int_equal(obs_encoder_audio_start_push_size(4096, 0), 4096);
	assert_int_equal(obs_encoder_audio_start_push_size(4096, 4), 4092);
	assert_int_equal(obs_encoder_audio_start_push_size(4096, 4092), 4);
	assert_int_equal(obs_encoder_audio_start_push_size(4096, 4095), 1);
	assert_int_equal(obs_encoder_audio_start_push_size(4096, 4096), 0);
	assert_int_equal(obs_encoder_audio_start_push_size(4096, 4097), 0);
	assert_int_equal(obs_encoder_audio_start_push_size(4096, SIZE_MAX), 0);
	assert_int_equal(obs_encoder_audio_start_push_size(0, 0), 0);
	assert_true(obs_encoder_audio_start_push_size(SIZE_MAX, 0) == SIZE_MAX);
	assert_true(obs_encoder_audio_start_push_size(SIZE_MAX, SIZE_MAX - 1) == 1);
}

static void can_send(void **state)
{
	(void)state;
	assert_false(obs_encoder_audio_start_can_send(4095, 4096));
	assert_true(obs_encoder_audio_start_can_send(4096, 4096));
	assert_true(obs_encoder_audio_start_can_send(4097, 4096));
	assert_false(obs_encoder_audio_start_can_send(0, 1));
	/* a frame of 0 bytes is always ready (characterized, not endorsed;
	 * framesize_bytes comes from the encoder's frame size) */
	assert_true(obs_encoder_audio_start_can_send(0, 0));
	assert_true(obs_encoder_audio_start_can_send(SIZE_MAX, SIZE_MAX));
	assert_false(obs_encoder_audio_start_can_send(SIZE_MAX - 1, SIZE_MAX));
}

static void audio_pts(void **state)
{
	(void)state;
	assert_int_equal(obs_encoder_audio_start_audio_pts(0, 1024), 1024);
	assert_int_equal(obs_encoder_audio_start_audio_pts(1024, 1024), 2048);
	assert_int_equal(obs_encoder_audio_start_audio_pts(-1024, 960), -64);
	assert_int_equal(obs_encoder_audio_start_audio_pts(5, 0), 5);
	/* computed in size_t and converted back: wraps (characterized, not
	 * endorsed) */
	assert_true(obs_encoder_audio_start_audio_pts(INT64_MAX, 1) == INT64_MIN);
	assert_true(obs_encoder_audio_start_audio_pts(-1, SIZE_MAX) == -2);
}

/* ------------------------------------------------------------------------ */
/* the setters */

static void setter_not_video(void **state)
{
	(void)state;
	assert_false(obs_encoder_audio_start_not_video(OBS_ENCODER_AUDIO_START_TYPE_VIDEO));
	assert_false(obs_encoder_audio_start_not_video(1));
	/* OBS_ENCODER_AUDIO and types outside the enum (a plugin's info.type is
	 * not range-checked) */
	assert_true(obs_encoder_audio_start_not_video(0));
	assert_true(obs_encoder_audio_start_not_video(2));
	assert_true(obs_encoder_audio_start_not_video(-1));
	assert_true(obs_encoder_audio_start_not_video(INT_MAX));
	assert_true(obs_encoder_audio_start_not_video(INT_MIN));
}

static void divisor_ok(void **state)
{
	(void)state;
	assert_false(obs_encoder_audio_start_divisor_ok(0));
	assert_true(obs_encoder_audio_start_divisor_ok(1));
	assert_true(obs_encoder_audio_start_divisor_ok(2));
	assert_true(obs_encoder_audio_start_divisor_ok(UINT32_MAX));
}

/* ------------------------------------------------------------------------ */
/* buffer_audio() end to end, on a stand-in encoder (obs-encoder.c's code,
 * with the deques reduced to the byte count of plane 0) */

struct stand_in {
	uint32_t samplerate;
	size_t blocksize;
	uint64_t start_ts;
	uint64_t first_raw_ts;
	bool has_paired;
	uint64_t v_start_ts;
	size_t buffered;
	/* what the last push_back_audio() pushed, and from where */
	size_t last_push;
	size_t last_offset;
	int pushes;
	int clears;
	int from_buffers;
};

static void si_push_back(struct stand_in *e, size_t size, size_t offset_size)
{
	size = obs_encoder_audio_start_push_size(size, offset_size);
	if (!size)
		return;
	e->buffered += size;
	e->last_push = size;
	e->last_offset = offset_size;
	e->pushes++;
}

static size_t si_calc_offset_size(struct stand_in *e, uint64_t v_start_ts, uint64_t a_start_ts)
{
	uint64_t offset = obs_encoder_audio_start_offset_ns(v_start_ts, a_start_ts);
	offset = util_mul_div64(offset, e->samplerate, 1000000000ULL);
	return obs_encoder_audio_start_offset_size(offset, e->blocksize);
}

static void si_start_from_buffer(struct stand_in *e, uint64_t v_start_ts)
{
	size_t size = e->buffered;
	size_t offset_size = 0;

	e->buffered = 0;
	e->from_buffers++;
	if (obs_encoder_audio_start_buffer_offset(e->first_raw_ts, v_start_ts))
		offset_size = si_calc_offset_size(e, v_start_ts, e->first_raw_ts);
	si_push_back(e, size, offset_size);
}

static bool si_buffer_audio(struct stand_in *e, uint64_t timestamp, uint32_t frames)
{
	size_t size = obs_encoder_audio_start_size(frames, e->blocksize);
	size_t offset_size = 0;
	bool success = true;

	if (obs_encoder_audio_start_paired(e->start_ts, e->has_paired)) {
		uint64_t v_start_ts = e->v_start_ts;
		struct obs_encoder_audio_start_plan plan;

		if (obs_encoder_audio_start_no_video(v_start_ts)) {
			success = false;
			goto fail;
		}

		plan = obs_encoder_audio_start_packet(timestamp, util_mul_div64(frames, 1000000000ULL, e->samplerate),
						      v_start_ts);
		if (plan.wait) {
			success = false;
			goto fail;
		}
		if (plan.truncate)
			offset_size = si_calc_offset_size(e, v_start_ts, timestamp);
		if (plan.clear) {
			e->buffered = 0;
			e->clears++;
		}
		e->start_ts = v_start_ts;
		if (plan.from_buffer)
			si_start_from_buffer(e, v_start_ts);
	} else if (obs_encoder_audio_start_unpaired(e->start_ts, e->has_paired)) {
		e->start_ts = timestamp;
	}

fail:
	si_push_back(e, size, offset_size);
	return success;
}

/* receive_audio()'s first packet: first_raw_ts and an emptied buffer. */
static struct stand_in si_new(uint32_t samplerate, size_t blocksize, bool paired, uint64_t v_start_ts,
			      uint64_t first_ts)
{
	struct stand_in e = {0};
	e.samplerate = samplerate;
	e.blocksize = blocksize;
	e.has_paired = paired;
	e.v_start_ts = v_start_ts;
	e.first_raw_ts = first_ts;
	return e;
}

static const size_t blocksizes[] = {4, 8};

/* One sample at 48 kHz is 20833.3 ns: 20834 ns converts to one frame. */
#define SAMPLE 20834ULL

static void e2e_before_by_one_sample(void **state)
{
	(void)state;
	for (size_t b = 0; b < 2; b++) {
		size_t bs = blocksizes[b];
		struct stand_in e = si_new(48000, bs, true, V, V - SAMPLE);
		assert_true(si_buffer_audio(&e, V - SAMPLE, 1024));
		assert_true(e.start_ts == V);
		assert_int_equal(e.clears, 1);
		assert_int_equal(e.from_buffers, 0);
		/* the first frame is dropped */
		assert_int_equal(e.last_offset, bs);
		assert_int_equal(e.buffered, 1023 * bs);
	}
	/* 20833 ns is less than one frame: nothing is dropped, though the
	 * packet starts before the video (characterized, not endorsed: the
	 * conversion rounds down) */
	struct stand_in e = si_new(48000, 4, true, V, V - SAMPLE + 1);
	assert_true(si_buffer_audio(&e, V - SAMPLE + 1, 1024));
	assert_int_equal(e.buffered, 4096);
	assert_int_equal(e.last_offset, 0);
}

static void e2e_at_video(void **state)
{
	(void)state;
	for (size_t b = 0; b < 2; b++) {
		size_t bs = blocksizes[b];
		struct stand_in e = si_new(48000, bs, true, V, V);
		e.buffered = 3 * bs; /* left from before: cleared */
		assert_true(si_buffer_audio(&e, V, 1024));
		assert_true(e.start_ts == V);
		assert_int_equal(e.clears, 1);
		assert_int_equal(e.buffered, 1024 * bs);
		assert_int_equal(e.last_offset, 0);
	}
}

/* The first packet came one packet before the video and waited (buffered
 * whole); the next, at the video, clears it. */
static void e2e_before_by_one_packet(void **state)
{
	(void)state;
	for (size_t b = 0; b < 2; b++) {
		size_t bs = blocksizes[b];
		struct stand_in e = si_new(48000, bs, true, V, V - PKT);
		assert_false(si_buffer_audio(&e, V - PKT, 1024));
		assert_true(e.start_ts == 0);
		assert_int_equal(e.buffered, 1024 * bs);
		assert_true(si_buffer_audio(&e, V, 1024));
		assert_true(e.start_ts == V);
		assert_int_equal(e.clears, 1);
		assert_int_equal(e.buffered, 1024 * bs);
	}
}

/* The packet ends exactly at the video's start: it waits (buffered whole). */
static void e2e_ends_at_video(void **state)
{
	(void)state;
	struct stand_in e = si_new(48000, 4, true, V, V - PKT);
	assert_false(si_buffer_audio(&e, V - PKT, 1024));
	assert_true(e.start_ts == 0);
	assert_int_equal(e.pushes, 1);
	assert_int_equal(e.buffered, 4096);
	/* one ns later it starts, its first 1023 frames dropped */
	e = si_new(48000, 4, true, V, V - PKT + 1);
	assert_true(si_buffer_audio(&e, V - PKT + 1, 1024));
	assert_true(e.start_ts == V);
	assert_int_equal(e.last_offset, 1023 * 4);
	assert_int_equal(e.buffered, 4);
}

/* The video started one sample, then one packet, after the first audio: the
 * packet after the video starts from the buffer, cut at the video. */
static void e2e_after_video(void **state)
{
	(void)state;
	for (size_t b = 0; b < 2; b++) {
		size_t bs = blocksizes[b];
		/* audio from V - PKT; the video at V - PKT + SAMPLE */
		struct stand_in e = si_new(48000, bs, true, 0, V - PKT);
		assert_false(si_buffer_audio(&e, V - PKT, 1024)); /* no video yet */
		assert_int_equal(e.buffered, 1024 * bs);
		e.v_start_ts = V - PKT + SAMPLE;
		assert_true(si_buffer_audio(&e, V, 1024));
		assert_true(e.start_ts == V - PKT + SAMPLE);
		assert_int_equal(e.from_buffers, 1);
		assert_int_equal(e.clears, 0);
		/* one frame of the buffer dropped, then the packet whole */
		assert_int_equal(e.buffered, 1023 * bs + 1024 * bs);

		/* audio from V - 2 PKT; the video at V - PKT (one packet after) */
		e = si_new(48000, bs, true, 0, V - 2 * PKT);
		assert_false(si_buffer_audio(&e, V - 2 * PKT, 1024));
		assert_false(si_buffer_audio(&e, V - PKT, 1024));
		assert_int_equal(e.buffered, 2048 * bs);
		e.v_start_ts = V - PKT;
		assert_true(si_buffer_audio(&e, V, 1024));
		assert_int_equal(e.from_buffers, 1);
		/* 21333333 ns is 1023 frames (rounded down): one frame of the
		 * first packet stays (characterized, not endorsed) */
		assert_int_equal(e.buffered, (2048 - 1023) * bs + 1024 * bs);
	}
}

/* start_from_buffer(): the offset below, at and above what is buffered. */
static void e2e_buffer_against_offset(void **state)
{
	(void)state;
	/* below: the buffer is shorter than the offset; it is dropped whole */
	struct stand_in e = si_new(48000, 4, true, V, V - 3 * PKT);
	e.buffered = 1024 * 4;
	e.start_ts = 0;
	assert_true(si_buffer_audio(&e, V + 1, 1024));
	assert_int_equal(e.from_buffers, 1);
	assert_int_equal(e.buffered, 4096);
	assert_int_equal(e.pushes, 1);

	/* at: the offset equals the buffer; nothing of it is pushed */
	e = si_new(48000, 4, true, V, V - 2 * SAMPLE);
	e.buffered = 2 * 4;
	assert_true(si_buffer_audio(&e, V + 1, 1024));
	assert_int_equal(e.buffered, 4096);
	assert_int_equal(e.pushes, 1);

	/* above: what is past the offset is kept */
	e = si_new(48000, 4, true, V, V - 2 * SAMPLE);
	e.buffered = 5 * 4;
	assert_true(si_buffer_audio(&e, V + 1, 1024));
	assert_int_equal(e.pushes, 2);
	assert_int_equal(e.buffered, 3 * 4 + 4096);

	/* the first audio at or after the video: no offset */
	e = si_new(48000, 8, true, V, V);
	e.buffered = 5 * 8;
	assert_true(si_buffer_audio(&e, V + 1, 1024));
	assert_int_equal(e.buffered, 5 * 8 + 8192);
}

static void e2e_unpaired_and_started(void **state)
{
	(void)state;
	/* unpaired: starts at its first packet, whatever its time */
	struct stand_in e = si_new(48000, 4, false, 0, V - 7);
	assert_true(si_buffer_audio(&e, V - 7, 1024));
	assert_true(e.start_ts == V - 7);
	assert_int_equal(e.buffered, 4096);
	assert_true(si_buffer_audio(&e, V + PKT, 1024));
	assert_true(e.start_ts == V - 7);
	assert_int_equal(e.buffered, 8192);

	/* started: every packet is buffered whole */
	e = si_new(44100, 8, true, V, V);
	e.start_ts = V;
	assert_true(si_buffer_audio(&e, V - 5 * PKT, 1024));
	assert_int_equal(e.buffered, 8192);
	assert_int_equal(e.clears, 0);
	assert_int_equal(e.from_buffers, 0);

	/* paired, no video yet: waits, buffered whole */
	e = si_new(48000, 4, true, 0, V);
	assert_false(si_buffer_audio(&e, V, 1024));
	assert_true(e.start_ts == 0);
	assert_int_equal(e.buffered, 4096);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(group_ready),
		cmocka_unit_test(paired_late),
		cmocka_unit_test(video_pts),
		cmocka_unit_test(packet_size),
		cmocka_unit_test(branches),
		cmocka_unit_test(no_video),
		cmocka_unit_test(packet_plan),
		cmocka_unit_test(offsets),
		cmocka_unit_test(push_size),
		cmocka_unit_test(can_send),
		cmocka_unit_test(audio_pts),
		cmocka_unit_test(setter_not_video),
		cmocka_unit_test(divisor_ok),
		cmocka_unit_test(e2e_before_by_one_sample),
		cmocka_unit_test(e2e_at_video),
		cmocka_unit_test(e2e_before_by_one_packet),
		cmocka_unit_test(e2e_ends_at_video),
		cmocka_unit_test(e2e_after_video),
		cmocka_unit_test(e2e_buffer_against_offset),
		cmocka_unit_test(e2e_unpaired_and_started),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
