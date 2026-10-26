#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-gpu-gates.h"
#include "obs-encoder-audio-start.h"
#include "media-io/video-io.h"

/*
 * Characterization of the per-frame and per-encoder gates of the GPU encode
 * thread (libobs/obs-video-gpu-encode.c), moved to
 * libobs/obs-encoder-gpu-gates.c before their Rust port:
 * - the frames skipped while the thread starts (obs_encoder_gpu_gates_warming,
 *   _warm_next);
 * - the frame rate divisor counter (_divisor_inc, _divisor_wrap,
 *   _divisor_skip), the keyed mutex key each encoder hands on (_next_key) and
 *   the frame timing entry (_record_timing);
 * - the requeue after the walk (_count_next, _requeue, _requeue_ts);
 * - init_gpu_encoding()'s texture format (_p010).
 * The thread also asks the gates receive_video() asks, ported in
 * obs-encoder-audio-start.c: the group start (obs_encoder_audio_start_group_ready),
 * the paired audio wait (_paired_late) and the pts step (_video_pts).
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The per-frame body of gpu_encode_thread() is also copied below onto
 * stand-in encoders (no thread, no lock, no texture), so the gates are pinned
 * end to end: two encoders started on the same frame with divisors 1, 2 and
 * 3 encode on the same frames; the keys handed on when every encoder encodes
 * and when one skips; the group and paired audio waits; the pts after a
 * failed encode; and a frame with a count of 1, 2 or 3, its timestamp near
 * UINT64_MAX.
 *
 * The functions are hidden in libobs, so this test compiles the C sources
 * into itself and calls only them, through their headers. The real-libobs
 * tests test_encoder_error_stop (a texture encoder through the thread,
 * including the teardown races), test_texture_encoder_raw_format and
 * test_encoder_join_keyframe cover the thread.
 *
 * Nothing here comes from outside OBS: timestamps come from the video
 * thread, counters and keys from libobs, the divisor from the frontend and
 * the format from the video settings.
 */

static const uint64_t edge_u64[] = {0, 1, 2, 0x7fffffffffffffffULL, 0x8000000000000000ULL, UINT64_MAX - 1, UINT64_MAX};
#define N_EDGE_U64 (sizeof(edge_u64) / sizeof(edge_u64[0]))
static const uint32_t edge_u32[] = {0, 1, 2, 3, 0x7fffffffU, 0x80000000U, UINT32_MAX - 1, UINT32_MAX};
#define N_EDGE_U32 (sizeof(edge_u32) / sizeof(edge_u32[0]))
static const size_t edge_size[] = {0, 1, 2, 3, 4, SIZE_MAX - 1, SIZE_MAX};
#define N_EDGE_SIZE (sizeof(edge_size) / sizeof(edge_size[0]))
/* INT_MIN is left out: count - 1 there overflows int (undefined) */
static const int edge_int[] = {INT_MIN + 1, -2, -1, 0, 1, 2, 3, INT_MAX - 1, INT_MAX};
#define N_EDGE_INT (sizeof(edge_int) / sizeof(edge_int[0]))

/* ------------------------------------------------------------------------ */
/* the constants */

static void constants(void **state)
{
	(void)state;
	/* NUM_ENCODE_TEXTURE_FRAMES_TO_WAIT: one frame is skipped */
	assert_int_equal(OBS_ENCODER_GPU_GATES_WAIT_FRAMES, 1);
	assert_int_equal(OBS_ENCODER_GPU_GATES_FORMAT_P010, (int)VIDEO_FORMAT_P010);
	assert_int_equal(OBS_ENCODER_GPU_GATES_FORMAT_P010, 18);
}

/* ------------------------------------------------------------------------ */
/* the start-up wait */

static void warmup(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_INT; i++) {
		assert_int_equal(obs_encoder_gpu_gates_warming(edge_int[i]), edge_int[i] != 0);
		assert_int_equal(obs_encoder_gpu_gates_warm_next(edge_int[i]), edge_int[i] - 1);
	}

	/* the thread skips frames while warming and counts down: exactly
	 * OBS_ENCODER_GPU_GATES_WAIT_FRAMES of them */
	int wait_frames = OBS_ENCODER_GPU_GATES_WAIT_FRAMES;
	int skipped = 0;
	for (int frame = 0; frame < 10; frame++) {
		if (obs_encoder_gpu_gates_warming(wait_frames)) {
			wait_frames = obs_encoder_gpu_gates_warm_next(wait_frames);
			skipped++;
		}
	}
	assert_int_equal(skipped, 1);
	assert_int_equal(wait_frames, 0);
}

/* ------------------------------------------------------------------------ */
/* the frame rate divisor counter */

static void divisor_gates(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_U32; i++) {
		/* a uint32_t sum: UINT32_MAX wraps to 0 */
		assert_true(obs_encoder_gpu_gates_divisor_inc(edge_u32[i]) == (uint32_t)(edge_u32[i] + 1U));
		assert_int_equal(obs_encoder_gpu_gates_divisor_skip(edge_u32[i]), edge_u32[i] != 0);
		for (size_t j = 0; j < N_EDGE_U32; j++)
			assert_int_equal(obs_encoder_gpu_gates_divisor_wrap(edge_u32[i], edge_u32[j]),
					 edge_u32[i] == edge_u32[j]);
	}
	assert_true(obs_encoder_gpu_gates_divisor_inc(UINT32_MAX) == 0);
}

/* ------------------------------------------------------------------------ */
/* the keys and the timing entry */

static void next_key(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_SIZE; i++) {
		for (size_t j = 0; j < N_EDGE_SIZE; j++) {
			for (size_t k = 0; k < N_EDGE_U64; k++) {
				uint64_t want = edge_size[i] == edge_size[j] ? 0 : edge_u64[k] + 1;
				assert_true(obs_encoder_gpu_gates_next_key(edge_size[i], edge_size[j], edge_u64[k]) ==
					    want);
			}
		}
	}
	/* the last encoder hands on 0, any other the next key; the key wraps */
	assert_true(obs_encoder_gpu_gates_next_key(3, 3, 41) == 0);
	assert_true(obs_encoder_gpu_gates_next_key(2, 3, 41) == 42);
	assert_true(obs_encoder_gpu_gates_next_key(1, 3, UINT64_MAX) == 0);
	/* characterized, not endorsed: lock_count past the number of encoders
	 * never happens (it counts the encoders that encode), and hands on the
	 * next key */
	assert_true(obs_encoder_gpu_gates_next_key(4, 3, 41) == 42);
}

static void record_timing(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_U64; i++)
		assert_int_equal(obs_encoder_gpu_gates_record_timing(edge_u64[i]), edge_u64[i] != 0);
}

/* ------------------------------------------------------------------------ */
/* the requeue */

static void requeue_gates(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_INT; i++) {
		assert_int_equal(obs_encoder_gpu_gates_count_next(edge_int[i]), edge_int[i] - 1);
		assert_int_equal(obs_encoder_gpu_gates_requeue(edge_int[i]), edge_int[i] != 0);
	}
	for (size_t i = 0; i < N_EDGE_U64; i++)
		for (size_t j = 0; j < N_EDGE_U64; j++)
			assert_true(obs_encoder_gpu_gates_requeue_ts(edge_u64[i], edge_u64[j]) ==
				    edge_u64[i] + edge_u64[j]);
	/* a uint64_t sum: near UINT64_MAX it wraps */
	assert_true(obs_encoder_gpu_gates_requeue_ts(UINT64_MAX - 5, 16666667) == 16666661);
}

/* ------------------------------------------------------------------------ */
/* init_gpu_encoding() */

static void texture_format(void **state)
{
	(void)state;
	for (int f = (int)VIDEO_FORMAT_NONE; f <= (int)VIDEO_FORMAT_R10L; f++)
		assert_int_equal(obs_encoder_gpu_gates_p010(f), f == (int)VIDEO_FORMAT_P010);
	/* every other value, NV12 included, gives NV12 textures */
	assert_false(obs_encoder_gpu_gates_p010((int)VIDEO_FORMAT_NV12));
	assert_false(obs_encoder_gpu_gates_p010(-1));
	assert_false(obs_encoder_gpu_gates_p010(INT_MAX));
	assert_false(obs_encoder_gpu_gates_p010(INT_MIN));
}

/* ------------------------------------------------------------------------ */
/* The shared gates as the GPU thread asks them */

static void shared_gates(void **state)
{
	(void)state;
	assert_true(obs_encoder_audio_start_group_ready(16666667, 16666667));
	assert_false(obs_encoder_audio_start_group_ready(16666667, 33333334));
	assert_true(obs_encoder_audio_start_paired_late(16666668, 16666667));
	assert_false(obs_encoder_audio_start_paired_late(16666667, 16666667));
	/* the pts step: timebase_num times the divisor, a uint32_t product */
	assert_true(obs_encoder_audio_start_video_pts(10, 1, 2) == 12);
	assert_true(obs_encoder_audio_start_video_pts(0, 0x10000, 0x10000) == 0);
}

/* ------------------------------------------------------------------------ */
/* The per-frame body of gpu_encode_thread(), copied onto stand-in encoders.
 * It keeps the C's order: the failed flag, the group start, the paired audio
 * wait (the walk stops at the first paired encoder not ready; a dead weak
 * reference is skipped), the pause check, the divisor counter, the start
 * timestamp, the key, the encode, the timing entry and the pts step; then the
 * requeue. */

#define SI_MAX_PAIRED 4
#define SI_MAX_CALLS 32

struct si_paired {
	bool alive;
	bool first_received;
	uint64_t first_raw_ts;
};

struct si_group {
	uint64_t start_timestamp;
};

struct si_enc {
	bool failed;
	struct si_group *group;
	uint64_t start_ts;
	bool first_received;
	size_t num_paired;
	struct si_paired paired[SI_MAX_PAIRED];
	bool paused;
	uint32_t frame_rate_divisor;
	uint32_t frame_rate_divisor_counter;
	uint32_t timebase_num;
	int64_t cur_pts;
	bool has_texture2;
	bool fail_encode;

	/* what the stand-in encode saw */
	size_t calls;
	uint64_t lock_key[SI_MAX_CALLS];
	uint64_t next_key[SI_MAX_CALLS];
	int64_t pts[SI_MAX_CALLS];
	uint64_t frame_ts[SI_MAX_CALLS];
	size_t texture2_calls;
	size_t timings;
};

struct si_frame {
	uint64_t timestamp;
	uint64_t lock_key;
	int count;
};

static bool si_encode(struct si_enc *e, uint64_t timestamp, uint64_t lock_key, uint64_t *next_key, bool texture2)
{
	if (e->calls < SI_MAX_CALLS) {
		e->lock_key[e->calls] = lock_key;
		e->next_key[e->calls] = *next_key;
		e->pts[e->calls] = e->cur_pts;
		e->frame_ts[e->calls] = timestamp;
	}
	e->calls++;
	if (texture2)
		e->texture2_calls++;
	return !e->fail_encode;
}

/* One pass of the thread over the frame; true: the frame goes back to the
 * front of the queue (with its new timestamp), false: to the available
 * queue. */
static bool si_pass(struct si_enc *encs, size_t num, struct si_frame *tf, uint64_t interval)
{
	uint64_t timestamp = tf->timestamp;
	uint64_t lock_key = tf->lock_key;
	uint64_t next_key = tf->lock_key;
	size_t lock_count = 0;

	for (size_t i = 0; i < num; i++) {
		struct si_enc *encoder = &encs[i];
		bool success;
		uint32_t skip;

		if (encoder->failed)
			continue;

		if (encoder->group && !encoder->start_ts) {
			if (!obs_encoder_audio_start_group_ready(encoder->group->start_timestamp, timestamp))
				continue;
		}

		if (!encoder->first_received && encoder->num_paired) {
			bool wait_for_audio = false;

			for (size_t idx = 0; !wait_for_audio && idx < encoder->num_paired; idx++) {
				struct si_paired *enc = &encoder->paired[idx];
				if (!enc->alive)
					continue;

				if (!enc->first_received ||
				    obs_encoder_audio_start_paired_late(enc->first_raw_ts, timestamp))
					wait_for_audio = true;
			}

			if (wait_for_audio)
				continue;
		}

		if (encoder->paused)
			continue;

		skip = encoder->frame_rate_divisor_counter;
		encoder->frame_rate_divisor_counter = obs_encoder_gpu_gates_divisor_inc(skip);
		if (obs_encoder_gpu_gates_divisor_wrap(encoder->frame_rate_divisor_counter,
						       encoder->frame_rate_divisor))
			encoder->frame_rate_divisor_counter = 0;
		if (obs_encoder_gpu_gates_divisor_skip(skip))
			continue;

		if (!encoder->start_ts)
			encoder->start_ts = timestamp;

		next_key = obs_encoder_gpu_gates_next_key(++lock_count, num, next_key);

		success = si_encode(encoder, timestamp, lock_key, &next_key, encoder->has_texture2);

		if (obs_encoder_gpu_gates_record_timing(tf->timestamp))
			encoder->timings++;

		/* send_off_encoder_packet(): a failed encode stops the encoder
		 * (full_stop() sets gpu_encode_failed) */
		if (!success)
			encoder->failed = true;

		lock_key = next_key;

		encoder->cur_pts = obs_encoder_audio_start_video_pts(encoder->cur_pts, encoder->timebase_num,
								     encoder->frame_rate_divisor);
	}

	tf->lock_key = next_key;

	tf->count = obs_encoder_gpu_gates_count_next(tf->count);
	if (obs_encoder_gpu_gates_requeue(tf->count)) {
		tf->timestamp = obs_encoder_gpu_gates_requeue_ts(tf->timestamp, interval);
		return true;
	}
	return false;
}

static void si_init(struct si_enc *e, uint32_t divisor)
{
	memset(e, 0, sizeof(*e));
	e->frame_rate_divisor = divisor;
	e->timebase_num = 1;
	e->first_received = true;
}

#define FRAME_NS 16666667ULL

/* Two encoders with the same divisor, started on the same frame, encode on
 * the same frames: the first frame and then every divisor-th one. That is why
 * the counter is explicit. */
static void e2e_divisors_in_step(void **state)
{
	(void)state;
	for (uint32_t d = 1; d <= 3; d++) {
		struct si_enc encs[2];
		si_init(&encs[0], d);
		si_init(&encs[1], d);

		for (uint64_t f = 0; f < 12; f++) {
			struct si_frame tf = {.timestamp = (f + 1) * FRAME_NS, .lock_key = 0, .count = 1};
			assert_false(si_pass(encs, 2, &tf, FRAME_NS));
		}
		for (size_t i = 0; i < 2; i++) {
			assert_int_equal(encs[i].calls, 12 / d);
			for (size_t c = 0; c < encs[i].calls; c++) {
				assert_true(encs[i].frame_ts[c] == (c * d + 1) * FRAME_NS);
				/* the pts steps by timebase_num times the divisor */
				assert_true(encs[i].pts[c] == (int64_t)(c * d));
			}
			assert_true(encs[i].start_ts == FRAME_NS);
			assert_int_equal(encs[i].frame_rate_divisor_counter, 12 % d);
		}
	}

	/* an encoder whose counter is one frame in is out of step: it encodes
	 * when the other skips */
	struct si_enc encs[2];
	si_init(&encs[0], 2);
	si_init(&encs[1], 2);
	encs[1].frame_rate_divisor_counter = 1;
	for (uint64_t f = 0; f < 4; f++) {
		struct si_frame tf = {.timestamp = (f + 1) * FRAME_NS, .lock_key = 0, .count = 1};
		si_pass(encs, 2, &tf, FRAME_NS);
	}
	assert_int_equal(encs[0].calls, 2);
	assert_int_equal(encs[1].calls, 2);
	assert_true(encs[0].frame_ts[0] == 1 * FRAME_NS);
	assert_true(encs[1].frame_ts[0] == 2 * FRAME_NS);

	/* characterized, not endorsed: a divisor of 0 (the setter refuses it)
	 * encodes the first frame and then never again until the uint32_t
	 * counter wraps */
	struct si_enc zero;
	si_init(&zero, 0);
	for (uint64_t f = 0; f < 5; f++) {
		struct si_frame tf = {.timestamp = (f + 1) * FRAME_NS, .lock_key = 0, .count = 1};
		si_pass(&zero, 1, &tf, FRAME_NS);
	}
	assert_int_equal(zero.calls, 1);
	assert_int_equal(zero.frame_rate_divisor_counter, 5);
	zero.frame_rate_divisor_counter = UINT32_MAX;
	struct si_frame tf = {.timestamp = 6 * FRAME_NS, .lock_key = 0, .count = 1};
	si_pass(&zero, 1, &tf, FRAME_NS);
	assert_int_equal(zero.calls, 1);
	assert_int_equal(zero.frame_rate_divisor_counter, 0);
	tf.timestamp = 7 * FRAME_NS;
	tf.count = 1;
	si_pass(&zero, 1, &tf, FRAME_NS);
	assert_int_equal(zero.calls, 2);
}

/* Every encoder takes the key the one before handed on; the last hands on 0,
 * which the frame keeps. */
static void e2e_keys_all_encode(void **state)
{
	(void)state;
	struct si_enc encs[3];
	for (size_t i = 0; i < 3; i++)
		si_init(&encs[i], 1);

	struct si_frame tf = {.timestamp = FRAME_NS, .lock_key = 5, .count = 1};
	assert_false(si_pass(encs, 3, &tf, FRAME_NS));
	assert_true(encs[0].lock_key[0] == 5 && encs[0].next_key[0] == 6);
	assert_true(encs[1].lock_key[0] == 6 && encs[1].next_key[0] == 7);
	assert_true(encs[2].lock_key[0] == 7 && encs[2].next_key[0] == 0);
	assert_true(tf.lock_key == 0);

	/* one encoder: key 0 in, 0 handed on */
	struct si_enc one;
	si_init(&one, 1);
	struct si_frame tf1 = {.timestamp = FRAME_NS, .lock_key = 0, .count = 1};
	si_pass(&one, 1, &tf1, FRAME_NS);
	assert_true(one.lock_key[0] == 0 && one.next_key[0] == 0);
	assert_true(tf1.lock_key == 0);

	/* the key wraps */
	struct si_enc two[2];
	si_init(&two[0], 1);
	si_init(&two[1], 1);
	struct si_frame tf2 = {.timestamp = FRAME_NS, .lock_key = UINT64_MAX, .count = 1};
	si_pass(two, 2, &tf2, FRAME_NS);
	assert_true(two[0].lock_key[0] == UINT64_MAX && two[0].next_key[0] == 0);
	assert_true(two[1].lock_key[0] == 0 && two[1].next_key[0] == 0);
}

/* Characterized, not endorsed (upstream): when an encoder in the list skips
 * the frame (divisor, failed, waiting or paused), lock_count never reaches the
 * number of encoders, so the last key handed on is not 0, and the frame keeps
 * it. */
static void e2e_keys_one_skips(void **state)
{
	(void)state;
	for (int why = 0; why < 4; why++) {
		struct si_enc encs[3];
		for (size_t i = 0; i < 3; i++)
			si_init(&encs[i], 1);
		switch (why) {
		case 0:
			encs[1].frame_rate_divisor = 2;
			encs[1].frame_rate_divisor_counter = 1;
			break;
		case 1:
			encs[1].failed = true;
			break;
		case 2:
			encs[1].paused = true;
			break;
		case 3:
			encs[1].first_received = false;
			encs[1].num_paired = 1;
			encs[1].paired[0].alive = true;
			break;
		}

		struct si_frame tf = {.timestamp = FRAME_NS, .lock_key = 5, .count = 1};
		si_pass(encs, 3, &tf, FRAME_NS);
		assert_int_equal(encs[1].calls, 0);
		assert_true(encs[0].lock_key[0] == 5 && encs[0].next_key[0] == 6);
		assert_true(encs[2].lock_key[0] == 6 && encs[2].next_key[0] == 7);
		assert_true(tf.lock_key == 7);
	}
}

/* An encoder in a group that has not started waits until the frame is the
 * group's start; once started it no longer asks. */
static void e2e_group_start(void **state)
{
	(void)state;
	struct si_group group = {.start_timestamp = 3 * FRAME_NS};
	struct si_enc e;
	si_init(&e, 1);
	e.group = &group;

	for (uint64_t f = 1; f <= 5; f++) {
		struct si_frame tf = {.timestamp = f * FRAME_NS, .lock_key = 0, .count = 1};
		si_pass(&e, 1, &tf, FRAME_NS);
	}
	assert_int_equal(e.calls, 3);
	assert_true(e.frame_ts[0] == 3 * FRAME_NS);
	assert_true(e.start_ts == 3 * FRAME_NS);

	/* a frame past the group's start never matches */
	struct si_enc late;
	si_init(&late, 1);
	late.group = &group;
	struct si_frame tf = {.timestamp = 4 * FRAME_NS, .lock_key = 0, .count = 1};
	si_pass(&late, 1, &tf, FRAME_NS);
	assert_int_equal(late.calls, 0);
}

/* The video waits while a paired audio encoder has not received audio or its
 * first audio is after the frame; a dead weak reference is skipped. */
static void e2e_paired_wait(void **state)
{
	(void)state;
	struct si_enc e;
	si_init(&e, 1);
	e.first_received = false;
	e.num_paired = 3;
	e.paired[0].alive = false;
	e.paired[1].alive = true;
	e.paired[1].first_received = true;
	e.paired[1].first_raw_ts = 2 * FRAME_NS;
	e.paired[2].alive = true;
	e.paired[2].first_received = true;
	e.paired[2].first_raw_ts = 3 * FRAME_NS;

	for (uint64_t f = 1; f <= 4; f++) {
		struct si_frame tf = {.timestamp = f * FRAME_NS, .lock_key = 0, .count = 1};
		si_pass(&e, 1, &tf, FRAME_NS);
	}
	/* frames 1 and 2 wait (paired[2] starts at 3), 3 and 4 encode */
	assert_int_equal(e.calls, 2);
	assert_true(e.frame_ts[0] == 3 * FRAME_NS);

	/* no audio yet on a live paired encoder: the video waits */
	struct si_enc w;
	si_init(&w, 1);
	w.first_received = false;
	w.num_paired = 1;
	w.paired[0].alive = true;
	struct si_frame tf = {.timestamp = FRAME_NS, .lock_key = 0, .count = 1};
	si_pass(&w, 1, &tf, FRAME_NS);
	assert_int_equal(w.calls, 0);

	/* only dead paired encoders: nothing to wait for */
	struct si_enc d;
	si_init(&d, 1);
	d.first_received = false;
	d.num_paired = 2;
	struct si_frame tf2 = {.timestamp = FRAME_NS, .lock_key = 0, .count = 1};
	si_pass(&d, 1, &tf2, FRAME_NS);
	assert_int_equal(d.calls, 1);
}

/* Characterized, not endorsed: the GPU path steps cur_pts after every
 * encode, failed or not; receive_video() steps it only after a successful
 * one. The failure stops the encoder either way, so the step is taken once. */
static void e2e_pts_after_failed_encode(void **state)
{
	(void)state;
	struct si_enc e;
	si_init(&e, 2);
	e.timebase_num = 1001;
	e.cur_pts = 7;
	e.fail_encode = true;
	for (uint64_t f = 1; f <= 4; f++) {
		struct si_frame tf = {.timestamp = f * FRAME_NS, .lock_key = 0, .count = 1};
		si_pass(&e, 1, &tf, FRAME_NS);
	}
	assert_int_equal(e.calls, 1);
	assert_true(e.pts[0] == 7);
	assert_true(e.failed);
	assert_true(e.cur_pts == 7 + 1001 * 2);
}

/* The timing entry is recorded only for a frame with a timestamp. */
static void e2e_timing(void **state)
{
	(void)state;
	struct si_enc e;
	si_init(&e, 1);
	struct si_frame tf = {.timestamp = 0, .lock_key = 0, .count = 1};
	si_pass(&e, 1, &tf, FRAME_NS);
	assert_int_equal(e.calls, 1);
	assert_int_equal(e.timings, 0);
	tf.timestamp = FRAME_NS;
	tf.count = 1;
	si_pass(&e, 1, &tf, FRAME_NS);
	assert_int_equal(e.calls, 2);
	assert_int_equal(e.timings, 1);
}

/* A frame with a count of n is encoded n times, one interval apart, then
 * goes to the available queue. */
static void e2e_requeue(void **state)
{
	(void)state;
	for (int count = 1; count <= 3; count++) {
		struct si_enc e;
		si_init(&e, 1);
		struct si_frame tf = {.timestamp = 10 * FRAME_NS, .lock_key = 0, .count = count};
		int passes = 1;
		while (si_pass(&e, 1, &tf, FRAME_NS))
			passes++;
		assert_int_equal(passes, count);
		assert_int_equal(tf.count, 0);
		assert_true(tf.timestamp == (10 + (uint64_t)count - 1) * FRAME_NS);
		assert_int_equal(e.calls, (size_t)count);
		for (int c = 0; c < count; c++)
			assert_true(e.frame_ts[c] == (10 + (uint64_t)c) * FRAME_NS);
	}

	/* near UINT64_MAX the timestamp wraps */
	struct si_enc e;
	si_init(&e, 1);
	struct si_frame tf = {.timestamp = UINT64_MAX - 5, .lock_key = 0, .count = 3};
	assert_true(si_pass(&e, 1, &tf, FRAME_NS));
	assert_true(tf.timestamp == FRAME_NS - 6);
	assert_int_equal(tf.count, 2);
	assert_true(si_pass(&e, 1, &tf, FRAME_NS));
	assert_true(tf.timestamp == 2 * FRAME_NS - 6);
	assert_false(si_pass(&e, 1, &tf, FRAME_NS));
	assert_true(tf.timestamp == 2 * FRAME_NS - 6);

	/* with no encoder the frame still counts down and requeues */
	struct si_frame none = {.timestamp = FRAME_NS, .lock_key = 9, .count = 2};
	assert_true(si_pass(NULL, 0, &none, FRAME_NS));
	assert_true(none.timestamp == 2 * FRAME_NS);
	assert_true(none.lock_key == 9);
	assert_false(si_pass(NULL, 0, &none, FRAME_NS));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(warmup),
		cmocka_unit_test(divisor_gates),
		cmocka_unit_test(next_key),
		cmocka_unit_test(record_timing),
		cmocka_unit_test(requeue_gates),
		cmocka_unit_test(texture_format),
		cmocka_unit_test(shared_gates),
		cmocka_unit_test(e2e_divisors_in_step),
		cmocka_unit_test(e2e_keys_all_encode),
		cmocka_unit_test(e2e_keys_one_skips),
		cmocka_unit_test(e2e_group_start),
		cmocka_unit_test(e2e_paired_wait),
		cmocka_unit_test(e2e_pts_after_failed_encode),
		cmocka_unit_test(e2e_timing),
		cmocka_unit_test(e2e_requeue),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
