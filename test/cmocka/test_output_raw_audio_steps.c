#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-raw-audio.h"
#include "obs-output-video-checks.h"
#include "util/util_uint64.h"

/*
 * Characterization of the raw output audio admission and the capture hookup
 * of libobs/obs-output.c, moved to libobs/obs-output-raw-audio.c before their
 * Rust port:
 * - prepare_audio(): an output without video takes the packet as it is
 *   (obs_output_raw_audio_passthrough); no video start yet, asked before and
 *   after it is taken from the pause data (_video_unset); a packet before
 *   the video (_before_video) that ends at or before it is dropped
 *   (_ends_before), else its front is cut (_cut_ns, _cut_ts, _cut_bytes,
 *   _cut_frames);
 * - default_raw_audio_callback(): the audio start (_start_unset), the bytes
 *   of a block and of a push (_block_bytes, _push_bytes), the block loop
 *   (_can_pop), each block's timestamp (_block_ts, _paused_ts) and the frame
 *   count (_next_frames);
 * - start_raw_audio(): each mix of the mask with raw_audio2 (_mix_on);
 * - hook_data_capture(): the encoded callback (_interleave) and the delay
 *   (_delay_on, _delay_ns).
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * prepare_audio(), the callback's block loop, start_raw_audio() and
 * hook_data_capture() are also copied below onto a stand-in output, so the
 * admission is pinned end to end against a video start at 48 kHz (before
 * it by one sample and by less than one, a packet ending exactly at it or
 * one nanosecond after, at it, after it, no video start yet, an audio-only
 * output), with the real util_mul_div64() as in obs-output.c; the block loop
 * holds back one block (characterized, not endorsed: the strict > keeps the
 * last full block buffered, and it is never sent when the output stops);
 * every mixer mask with and without raw_audio2 (the first mixer is the real
 * obs_output_first_mixer()); and every flag combination of the hookup with
 * and without a delay.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. The real-libobs
 * tests (test_raw_output_video_reset, test_output_before_video,
 * test_output_delay_e2e, test_output_delay_restart,
 * test_output_interleave_e2e, test_output_input_checks,
 * test_replay_buffer_pause_stop, test_ffmpeg_output_drain) cover the
 * callers.
 *
 * Nothing here comes from outside OBS: timestamps come from the audio and
 * video threads, sizes, rates, the mixer mask and the delay from libobs and
 * the frontend, the flags from an output plugin.
 */

#define NS 1000000000ULL
/* one 1024-frame packet at 48 kHz: util_mul_div64(1024, NS, 48000) */
#define PKT 21333333ULL

static const uint64_t edge_u64[] = {0, 1, 2, 0x7fffffffffffffffULL, 0x8000000000000000ULL, UINT64_MAX - 1, UINT64_MAX};
#define N_EDGE_U64 (sizeof(edge_u64) / sizeof(edge_u64[0]))

static void constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_OUTPUT_RAW_AUDIO_FLAG_VIDEO, 1);
	assert_int_equal(OBS_OUTPUT_RAW_AUDIO_FRAMES, 1024);
}

/* ------------------------------------------------------------------------ */
/* prepare_audio() */

static void passthrough(void **state)
{
	(void)state;
	for (uint32_t f = 0; f < 0x10000; f++)
		assert_int_equal(obs_output_raw_audio_passthrough(f), (f & 1) == 0);
	assert_false(obs_output_raw_audio_passthrough(UINT32_MAX));
	assert_true(obs_output_raw_audio_passthrough(UINT32_MAX - 1));
	/* OBS_OUTPUT_AUDIO alone (2), audio and encoded (6) pass */
	assert_true(obs_output_raw_audio_passthrough(2));
	assert_true(obs_output_raw_audio_passthrough(6));
	assert_false(obs_output_raw_audio_passthrough(3));
}

static void unset(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_U64; i++) {
		assert_int_equal(obs_output_raw_audio_video_unset(edge_u64[i]), edge_u64[i] == 0);
		assert_int_equal(obs_output_raw_audio_start_unset(edge_u64[i]), edge_u64[i] == 0);
	}
}

static void before_video(void **state)
{
	(void)state;
	for (size_t i = 0; i < N_EDGE_U64; i++)
		for (size_t j = 0; j < N_EDGE_U64; j++)
			assert_int_equal(obs_output_raw_audio_before_video(edge_u64[i], edge_u64[j]),
					 edge_u64[i] < edge_u64[j]);
	assert_true(obs_output_raw_audio_before_video(NS - 1, NS));
	assert_false(obs_output_raw_audio_before_video(NS, NS));
	assert_false(obs_output_raw_audio_before_video(NS + 1, NS));
}

static void ends_before(void **state)
{
	(void)state;
	/* a packet ending exactly at the video start is dropped */
	assert_true(obs_output_raw_audio_ends_before(NS - PKT, PKT, NS));
	assert_false(obs_output_raw_audio_ends_before(NS - PKT + 1, PKT, NS));
	assert_true(obs_output_raw_audio_ends_before(NS - 2 * PKT, PKT, NS));
	/* a 0-frame packet before the video ends before it */
	assert_true(obs_output_raw_audio_ends_before(NS - 1, 0, NS));
	/* the end wraps past UINT64_MAX and so is "before" the video
	 * (characterized, not endorsed; timestamps never get there) */
	assert_true(obs_output_raw_audio_ends_before(UINT64_MAX - 1, 3, UINT64_MAX));
	assert_true(obs_output_raw_audio_ends_before(10, UINT64_MAX, 100));
	for (size_t i = 0; i < N_EDGE_U64; i++)
		for (size_t j = 0; j < N_EDGE_U64; j++)
			for (size_t k = 0; k < N_EDGE_U64; k++)
				assert_int_equal(obs_output_raw_audio_ends_before(edge_u64[i], edge_u64[j],
										  edge_u64[k]),
						 (uint64_t)(edge_u64[i] + edge_u64[j]) <= edge_u64[k]);
}

static void cut(void **state)
{
	(void)state;
	assert_int_equal(obs_output_raw_audio_cut_ns(NS, NS - 20834), 20834);
	assert_int_equal(obs_output_raw_audio_cut_ns(NS, NS), 0);
	/* uint64_t difference, wrapping (only asked with timestamp before) */
	assert_true(obs_output_raw_audio_cut_ns(0, 1) == UINT64_MAX);
	assert_int_equal(obs_output_raw_audio_cut_ts(NS - 20834, 20834), NS);
	assert_true(obs_output_raw_audio_cut_ts(UINT64_MAX, 2) == 1);

	assert_int_equal(obs_output_raw_audio_cut_bytes(4, 1), 4);
	assert_int_equal(obs_output_raw_audio_cut_bytes(8, 1023), 8184);
	assert_int_equal(obs_output_raw_audio_cut_bytes(0, 1000), 0);
	assert_int_equal(obs_output_raw_audio_cut_bytes(4, 0), 0);
	/* the frames are truncated to uint32_t first (characterized, not
	 * endorsed; a cut is always shorter than its packet) */
	assert_int_equal(obs_output_raw_audio_cut_bytes(4, 0x100000001ULL), 4);
	assert_int_equal(obs_output_raw_audio_cut_bytes(4, 0x100000000ULL), 0);
#if SIZE_MAX > UINT32_MAX
	/* a size_t product: no 32-bit wrap */
	assert_true(obs_output_raw_audio_cut_bytes(8, UINT32_MAX) == (size_t)UINT32_MAX * 8);
#endif

	assert_int_equal(obs_output_raw_audio_cut_frames(1024, 1), 1023);
	assert_int_equal(obs_output_raw_audio_cut_frames(1024, 1023), 1);
	assert_int_equal(obs_output_raw_audio_cut_frames(1024, 0), 1024);
	/* uint32_t, wrapping, after the same truncation */
	assert_int_equal(obs_output_raw_audio_cut_frames(1024, 1025), UINT32_MAX);
	assert_int_equal(obs_output_raw_audio_cut_frames(1024, 0x100000001ULL), 1023);
}

/* ------------------------------------------------------------------------ */
/* default_raw_audio_callback() */

static void block_bytes(void **state)
{
	(void)state;
	assert_int_equal(obs_output_raw_audio_block_bytes(4), 4096);
	assert_int_equal(obs_output_raw_audio_block_bytes(8), 8192);
	assert_int_equal(obs_output_raw_audio_block_bytes(0), 0);
	assert_true(obs_output_raw_audio_block_bytes(SIZE_MAX) == (size_t)0 - 1024);

	assert_int_equal(obs_output_raw_audio_push_bytes(1024, 4), 4096);
	assert_int_equal(obs_output_raw_audio_push_bytes(480, 8), 3840);
	assert_int_equal(obs_output_raw_audio_push_bytes(0, 8), 0);
	assert_int_equal(obs_output_raw_audio_push_bytes(1, 0), 0);
#if SIZE_MAX > UINT32_MAX
	assert_true(obs_output_raw_audio_push_bytes(UINT32_MAX, 8) == (size_t)UINT32_MAX * 8);
#endif
}

static void can_pop(void **state)
{
	(void)state;
	/* strictly more than one block: exactly one block stays buffered
	 * (characterized, not endorsed) */
	assert_false(obs_output_raw_audio_can_pop(4096, 4096));
	assert_true(obs_output_raw_audio_can_pop(4097, 4096));
	assert_false(obs_output_raw_audio_can_pop(4095, 4096));
	assert_true(obs_output_raw_audio_can_pop(8192, 4096));
	assert_false(obs_output_raw_audio_can_pop(0, 0));
	/* a block of 0 bytes pops while anything is buffered */
	assert_true(obs_output_raw_audio_can_pop(1, 0));
	assert_false(obs_output_raw_audio_can_pop(SIZE_MAX, SIZE_MAX));
}

static void block_ts(void **state)
{
	(void)state;
	assert_int_equal(obs_output_raw_audio_block_ts(NS, 0), NS);
	assert_int_equal(obs_output_raw_audio_block_ts(NS, PKT), NS + PKT);
	assert_true(obs_output_raw_audio_block_ts(UINT64_MAX, 1) == 0);
	assert_int_equal(obs_output_raw_audio_paused_ts(NS, 0), NS);
	assert_int_equal(obs_output_raw_audio_paused_ts(NS, 5 * NS), 6 * NS);
	assert_true(obs_output_raw_audio_paused_ts(UINT64_MAX, 2) == 1);

	assert_int_equal(obs_output_raw_audio_next_frames(0), 1024);
	assert_int_equal(obs_output_raw_audio_next_frames(1024), 2048);
	assert_true(obs_output_raw_audio_next_frames(SIZE_MAX) == 1023);
}

/* ------------------------------------------------------------------------ */
/* start_raw_audio() and hook_data_capture() */

static void mix_on(void **state)
{
	(void)state;
	for (size_t mask = 0; mask < 0x100; mask++)
		for (size_t idx = 0; idx < 8; idx++)
			assert_int_equal(obs_output_raw_audio_mix_on(mask, idx), (mask >> idx) & 1);
	assert_true(obs_output_raw_audio_mix_on(SIZE_MAX, 5));
	assert_false(obs_output_raw_audio_mix_on(SIZE_MAX - 32, 5));
}

static void hook_rules(void **state)
{
	(void)state;
	assert_true(obs_output_raw_audio_interleave(true, true));
	assert_false(obs_output_raw_audio_interleave(true, false));
	assert_false(obs_output_raw_audio_interleave(false, true));
	assert_false(obs_output_raw_audio_interleave(false, false));

	assert_false(obs_output_raw_audio_delay_on(0));
	assert_true(obs_output_raw_audio_delay_on(1));
	assert_true(obs_output_raw_audio_delay_on(UINT32_MAX));

	assert_int_equal(obs_output_raw_audio_delay_ns(0), 0);
	assert_int_equal(obs_output_raw_audio_delay_ns(1), NS);
	assert_int_equal(obs_output_raw_audio_delay_ns(30), 30 * NS);
	/* a uint64_t product: no 32-bit wrap */
	assert_true(obs_output_raw_audio_delay_ns(UINT32_MAX) == (uint64_t)UINT32_MAX * NS);
}

/* ------------------------------------------------------------------------ */
/* The callers, copied onto a stand-in output */

#define SI_PLANES 8
#define SI_MAX_BLOCKS 16

struct stand_in {
	uint32_t flags;
	uint64_t video_start_ts;
	uint64_t last_video_ts; /* pause.last_video_ts */
	uint64_t ts_offset;     /* pause.ts_offset */
	uint64_t audio_start_ts;
	size_t sample_rate;
	size_t audio_size;
	size_t planes;
	size_t total_audio_frames;
	size_t buffered[SI_PLANES]; /* bytes in each plane's deque */
	/* the blocks sent to raw_audio */
	size_t blocks;
	uint64_t block_ts[SI_MAX_BLOCKS];
};

struct si_packet {
	uint64_t timestamp;
	uint32_t frames;
	size_t advance; /* bytes data[0] moved forward */
};

/* prepare_audio() */
static bool si_prepare(struct stand_in *o, const struct si_packet *old, struct si_packet *new)
{
	if (obs_output_raw_audio_passthrough(o->flags)) {
		*new = *old;
		return true;
	}

	if (obs_output_raw_audio_video_unset(o->video_start_ts))
		o->video_start_ts = o->last_video_ts;

	if (obs_output_raw_audio_video_unset(o->video_start_ts))
		return false;

	*new = *old;

	if (obs_output_raw_audio_before_video(old->timestamp, o->video_start_ts)) {
		uint64_t duration = util_mul_div64(old->frames, NS, o->sample_rate);
		uint64_t cutoff;

		if (obs_output_raw_audio_ends_before(old->timestamp, duration, o->video_start_ts))
			return false;

		cutoff = obs_output_raw_audio_cut_ns(o->video_start_ts, old->timestamp);
		new->timestamp = obs_output_raw_audio_cut_ts(new->timestamp, cutoff);

		cutoff = util_mul_div64(cutoff, o->sample_rate, NS);

		new->advance += obs_output_raw_audio_cut_bytes(o->audio_size, cutoff);
		new->frames = obs_output_raw_audio_cut_frames(new->frames, cutoff);
	}

	return true;
}

/* default_raw_audio_callback(), with data_active() true and no pause */
static bool si_callback(struct stand_in *o, uint64_t timestamp, uint32_t frames, struct si_packet *out)
{
	struct si_packet in = {timestamp, frames, 0};
	size_t frame_size_bytes;

	if (!si_prepare(o, &in, out))
		return false;
	if (obs_output_raw_audio_start_unset(o->audio_start_ts))
		o->audio_start_ts = out->timestamp;

	frame_size_bytes = obs_output_raw_audio_block_bytes(o->audio_size);

	for (size_t i = 0; i < o->planes; i++)
		o->buffered[i] += obs_output_raw_audio_push_bytes(out->frames, o->audio_size);

	while (obs_output_raw_audio_can_pop(o->buffered[0], frame_size_bytes)) {
		for (size_t i = 0; i < o->planes; i++)
			o->buffered[i] -= frame_size_bytes;

		uint64_t ts = obs_output_raw_audio_block_ts(o->audio_start_ts,
							    util_mul_div64(o->total_audio_frames, NS, o->sample_rate));
		ts = obs_output_raw_audio_paused_ts(ts, o->ts_offset);
		o->total_audio_frames = obs_output_raw_audio_next_frames(o->total_audio_frames);

		assert_true(o->blocks < SI_MAX_BLOCKS);
		o->block_ts[o->blocks++] = ts;
	}
	return true;
}

static struct stand_in si_new(uint32_t flags, uint64_t video_start_ts, uint64_t last_video_ts)
{
	struct stand_in o = {0};
	o.flags = flags;
	o.video_start_ts = video_start_ts;
	o.last_video_ts = last_video_ts;
	o.sample_rate = 48000;
	o.audio_size = 4; /* float */
	o.planes = 2;
	return o;
}

#define V (10 * NS)
#define AV 3 /* OBS_OUTPUT_AV */

static void e2e_before_by_one_sample(void **state)
{
	(void)state;
	struct si_packet out;
	/* 20834 ns is just over one sample at 48 kHz: one frame dropped */
	struct stand_in o = si_new(AV, V, 0);
	assert_true(si_callback(&o, V - 20834, 1024, &out));
	assert_int_equal(out.timestamp, V);
	assert_int_equal(out.frames, 1023);
	assert_int_equal(out.advance, 4);
	assert_int_equal(o.audio_start_ts, V);
	assert_int_equal(o.buffered[0], 1023 * 4);
	assert_int_equal(o.buffered[1], 1023 * 4);

	/* 20833 ns is under one sample: the timestamp moves to the video's,
	 * no frame is dropped (characterized, not endorsed: the conversion
	 * rounds down) */
	o = si_new(AV, V, 0);
	assert_true(si_callback(&o, V - 20833, 1024, &out));
	assert_int_equal(out.timestamp, V);
	assert_int_equal(out.frames, 1024);
	assert_int_equal(out.advance, 0);
}

static void e2e_ends_at_video(void **state)
{
	(void)state;
	struct si_packet out;
	/* a packet ending exactly at the video start is dropped */
	struct stand_in o = si_new(AV, V, 0);
	assert_false(si_callback(&o, V - PKT, 1024, &out));
	assert_int_equal(o.audio_start_ts, 0);
	assert_int_equal(o.buffered[0], 0);
	/* and one a packet earlier */
	assert_false(si_callback(&o, V - 2 * PKT, 1024, &out));

	/* one nanosecond later: 1023 frames are cut, one is kept (the
	 * conversion rounds down) */
	o = si_new(AV, V, 0);
	assert_true(si_callback(&o, V - PKT + 1, 1024, &out));
	assert_int_equal(out.timestamp, V);
	assert_int_equal(out.frames, 1);
	assert_int_equal(out.advance, 1023 * 4);
	assert_int_equal(o.buffered[0], 4);
}

static void e2e_at_and_after_video(void **state)
{
	(void)state;
	struct si_packet out;
	struct stand_in o = si_new(AV, V, 0);
	assert_true(si_callback(&o, V, 1024, &out));
	assert_int_equal(out.timestamp, V);
	assert_int_equal(out.frames, 1024);
	assert_int_equal(out.advance, 0);

	o = si_new(AV, V, 0);
	assert_true(si_callback(&o, V + 1, 1024, &out));
	assert_int_equal(out.timestamp, V + 1);
	assert_int_equal(out.frames, 1024);
	assert_int_equal(o.audio_start_ts, V + 1);
}

static void e2e_no_video_start(void **state)
{
	(void)state;
	struct si_packet out;
	/* no video yet: the packet waits, nothing is buffered */
	struct stand_in o = si_new(AV, 0, 0);
	assert_false(si_callback(&o, V, 1024, &out));
	assert_int_equal(o.video_start_ts, 0);
	assert_int_equal(o.buffered[0], 0);

	/* the video start comes from the pause data's last video timestamp */
	o = si_new(AV, 0, V);
	assert_true(si_callback(&o, V - 20834, 1024, &out));
	assert_int_equal(o.video_start_ts, V);
	assert_int_equal(out.frames, 1023);

	/* once set it stays, whatever the pause data says later */
	o.last_video_ts = 2 * V;
	assert_true(si_callback(&o, V + PKT, 1024, &out));
	assert_int_equal(o.video_start_ts, V);
}

static void e2e_audio_only(void **state)
{
	(void)state;
	struct si_packet out;
	/* an output without video takes every packet whole, video start or
	 * not */
	struct stand_in o = si_new(2, 0, 0);
	assert_true(si_callback(&o, 5, 1024, &out));
	assert_int_equal(out.timestamp, 5);
	assert_int_equal(out.frames, 1024);
	assert_int_equal(o.video_start_ts, 0);
	assert_int_equal(o.audio_start_ts, 5);

	o = si_new(2, V, V);
	assert_true(si_callback(&o, V - 2 * PKT, 1024, &out));
	assert_int_equal(out.timestamp, V - 2 * PKT);
	assert_int_equal(out.frames, 1024);
}

static void e2e_hold_back_one_block(void **state)
{
	(void)state;
	struct si_packet out;
	struct stand_in o = si_new(2, 0, 0);
	o.ts_offset = 7;
	/* one block buffered: nothing sent (characterized, not endorsed: the
	 * strict > holds the last full block back, and nothing sends it when
	 * the output stops) */
	assert_true(si_callback(&o, V, 1024, &out));
	assert_int_equal(o.blocks, 0);
	assert_int_equal(o.buffered[0], 4096);
	/* two: one sent, at the audio start plus the pause offset */
	assert_true(si_callback(&o, V + PKT, 1024, &out));
	assert_int_equal(o.blocks, 1);
	assert_int_equal(o.block_ts[0], V + 7);
	assert_int_equal(o.buffered[0], 4096);
	assert_int_equal(o.buffered[1], 4096);
	assert_int_equal(o.total_audio_frames, 1024);
	/* three: two sent, the second a block later */
	assert_true(si_callback(&o, V + 2 * PKT, 1024, &out));
	assert_int_equal(o.blocks, 2);
	assert_int_equal(o.block_ts[1], V + PKT + 7);
	assert_int_equal(o.total_audio_frames, 2048);

	/* 1.5 blocks in one packet: one sent, half a block kept; 2.5: two
	 * sent, half kept */
	o = si_new(2, 0, 0);
	assert_true(si_callback(&o, V, 1536, &out));
	assert_int_equal(o.blocks, 1);
	assert_int_equal(o.buffered[0], 512 * 4);
	o = si_new(2, 0, 0);
	assert_true(si_callback(&o, V, 2560, &out));
	assert_int_equal(o.blocks, 2);
	assert_int_equal(o.buffered[0], 512 * 4);
	/* three blocks in one packet: two sent */
	o = si_new(2, 0, 0);
	assert_true(si_callback(&o, V, 3072, &out));
	assert_int_equal(o.blocks, 2);
	assert_int_equal(o.buffered[0], 4096);
	/* the block timestamps count from the first packet, not from each */
	assert_int_equal(o.block_ts[0], V);
	assert_int_equal(o.block_ts[1], V + PKT);
}

/* start_raw_audio(): the mixes connected */
static size_t si_mixers(size_t mixer_mask, bool raw_audio2, size_t *mixes)
{
	size_t n = 0;
	if (raw_audio2) {
		for (int idx = 0; idx < OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_MIXES; idx++) {
			if (obs_output_raw_audio_mix_on(mixer_mask, (size_t)idx))
				mixes[n++] = (size_t)idx;
		}
	} else {
		mixes[n++] = obs_output_first_mixer(mixer_mask);
	}
	return n;
}

static void e2e_mixers(void **state)
{
	(void)state;
	size_t mixes[8];
	for (size_t mask = 0; mask < 0x100; mask++) {
		size_t n = si_mixers(mask, true, mixes);
		size_t k = 0;
		for (size_t idx = 0; idx < 6; idx++)
			if (mask & ((size_t)1 << idx))
				assert_int_equal(mixes[k++], idx);
		assert_int_equal(n, k);

		n = si_mixers(mask, false, mixes);
		assert_int_equal(n, 1);
		size_t first = 0;
		for (size_t idx = 0; idx < 6; idx++)
			if (mask & ((size_t)1 << idx)) {
				first = idx;
				break;
			}
		assert_int_equal(mixes[0], first);
	}
	/* no mix in the mask: raw_audio2 connects none, raw_audio mix 0
	 * (characterized, not endorsed) */
	assert_int_equal(si_mixers(0, true, mixes), 0);
	assert_int_equal(si_mixers(0, false, mixes), 1);
	assert_int_equal(mixes[0], 0);
	/* bits past MAX_AUDIO_MIXES are ignored by both */
	assert_int_equal(si_mixers(0xc0, true, mixes), 0);
	assert_int_equal(si_mixers(0xc0, false, mixes), 1);
	assert_int_equal(mixes[0], 0);
}

/* hook_data_capture() */
enum si_cb { SI_DEFAULT, SI_INTERLEAVE };
struct si_hook {
	bool encoded;
	enum si_cb cb; /* the callback the encoders (or the delay) send to */
	bool delay;    /* process_delay() wraps cb */
	uint64_t delay_ns;
	bool audio; /* audio encoders started, or raw audio connected */
	bool video; /* video encoders started, or raw video started */
};

static struct si_hook si_hook_data_capture(uint32_t flags, uint32_t delay_sec)
{
	struct si_hook h = {0};
	bool has_video = (flags & 1) != 0;
	bool has_audio = (flags & 2) != 0;

	if ((flags & 4) != 0) {
		h.encoded = true;
		h.cb = obs_output_raw_audio_interleave(has_video, has_audio) ? SI_INTERLEAVE : SI_DEFAULT;
		if (obs_output_raw_audio_delay_on(delay_sec)) {
			h.delay = true;
			h.delay_ns = obs_output_raw_audio_delay_ns(delay_sec);
		}
	}
	h.audio = has_audio;
	h.video = has_video;
	return h;
}

static void e2e_hook(void **state)
{
	(void)state;
	static const uint32_t delays[] = {0, 1, 20, UINT32_MAX};
	for (uint32_t flags = 0; flags < 16; flags++) {
		for (size_t d = 0; d < 4; d++) {
			struct si_hook h = si_hook_data_capture(flags, delays[d]);
			bool enc = (flags & 4) != 0;
			assert_int_equal(h.encoded, enc);
			assert_int_equal(h.cb, enc && (flags & 3) == 3 ? SI_INTERLEAVE : SI_DEFAULT);
			/* the delay applies only to an encoded output */
			assert_int_equal(h.delay, enc && delays[d] != 0);
			assert_true(h.delay_ns == (enc ? (uint64_t)delays[d] * NS : 0));
			assert_int_equal(h.audio, (flags & 2) != 0);
			assert_int_equal(h.video, (flags & 1) != 0);
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(passthrough),
		cmocka_unit_test(unset),
		cmocka_unit_test(before_video),
		cmocka_unit_test(ends_before),
		cmocka_unit_test(cut),
		cmocka_unit_test(block_bytes),
		cmocka_unit_test(can_pop),
		cmocka_unit_test(block_ts),
		cmocka_unit_test(mix_on),
		cmocka_unit_test(hook_rules),
		cmocka_unit_test(e2e_before_by_one_sample),
		cmocka_unit_test(e2e_ends_at_video),
		cmocka_unit_test(e2e_at_and_after_video),
		cmocka_unit_test(e2e_no_video_start),
		cmocka_unit_test(e2e_audio_only),
		cmocka_unit_test(e2e_hold_back_one_block),
		cmocka_unit_test(e2e_mixers),
		cmocka_unit_test(e2e_hook),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
