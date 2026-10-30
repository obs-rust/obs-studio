#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-pause.h"
#include "util/util_uint64.h"

/*
 * Characterization of the pause checks of libobs/obs-encoder.c, moved to
 * libobs/obs-encoder-pause.c before their Rust port (#3033):
 * - video_pause_check(): obs_encoder_pause_off() and
 *   obs_encoder_pause_video_end() and obs_encoder_pause_video_drop(),
 *   whether a frame ends the pause, is dropped or passes;
 * - audio_pause_check(): obs_encoder_pause_off(),
 *   obs_encoder_pause_audio_end(), obs_encoder_pause_audio_start() and
 *   obs_encoder_pause_audio_paused(), where a buffer is cut, dropped or
 *   passed; obs_encoder_pause_audio_cut_ns(), _kept(), _left() and
 *   _drop() for each cut.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. video() and audio()
 * below ask them as obs-encoder.c does, on a stand-in for the pause record
 * and one float planar buffer; obs-encoder.c keeps the mutex, the
 * last_video_ts store, both conversions, the record and buffer writes, the
 * plane advance and the ts_offset subtraction.
 *
 * Not covered: a video frame after a set pause end that never lands on it
 * (an encoder with a frame rate divisor), and a paused audio buffer that
 * starts after a set pause end (a pause and unpause inside one buffer). The
 * C drops both and every frame or buffer after them; the Rust port ends the
 * pause there on purpose, and its parity test lists them.
 */

#define RATE 48000
#define FRAMES 1024
/* audio_frames_to_ns(48000, 1024): 21333333.33 ns, truncated. */
#define BUFFER_NS 21333333ULL
#define T0 1000000000ULL

/* ------------------------------------------------------------------------ */
/* the record and the callers, as obs-encoder.c asks the moved C */

struct record {
	uint64_t last_video_ts;
	uint64_t ts_start;
	uint64_t ts_end;
	uint64_t ts_offset;
};

struct buffer {
	uint8_t *data[2];
	uint32_t frames;
	uint64_t timestamp;
};

static float plane0[FRAMES];
static float plane1[FRAMES];

static void fill(struct buffer *b, uint32_t frames, uint64_t timestamp)
{
	b->data[0] = (uint8_t *)plane0;
	b->data[1] = (uint8_t *)plane1;
	b->frames = frames;
	b->timestamp = timestamp;
}

/* obs-output.c's end_pause(). */
static void end(struct record *r, uint64_t ts)
{
	r->ts_end = ts;
	r->ts_offset += r->ts_end - r->ts_start;
}

static bool video(struct record *r, uint64_t ts)
{
	r->last_video_ts = ts;
	if (obs_encoder_pause_off(r->ts_start))
		return false;

	if (obs_encoder_pause_video_end(r->ts_end, ts)) {
		r->ts_start = 0;
		r->ts_end = 0;
	} else if (obs_encoder_pause_video_drop(r->ts_start, ts)) {
		return true;
	}
	return false;
}

static uint64_t cut_frames(uint64_t ts, uint64_t timestamp)
{
	return util_mul_div64(obs_encoder_pause_audio_cut_ns(ts, timestamp), RATE, 1000000000ULL);
}

static bool audio_internal(struct record *r, struct buffer *b)
{
	if (obs_encoder_pause_off(r->ts_start))
		return false;

	uint64_t end_ts = obs_encoder_pause_audio_end(b->timestamp, util_mul_div64(b->frames, 1000000000ULL, RATE));

	int start = obs_encoder_pause_audio_start(r->ts_start, b->timestamp, end_ts);
	if (start == OBS_ENCODER_PAUSE_AUDIO_START_INSIDE) {
		b->frames = obs_encoder_pause_audio_kept(cut_frames(r->ts_start, b->timestamp));
		return obs_encoder_pause_audio_drop(b->frames);
	} else if (start == OBS_ENCODER_PAUSE_AUDIO_START_BEFORE) {
		int paused = obs_encoder_pause_audio_paused(r->ts_end, b->timestamp, end_ts);
		if (paused == OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE) {
			uint64_t cut = cut_frames(r->ts_end, b->timestamp);
			for (size_t i = 0; i < 2; i++)
				b->data[i] += cut * sizeof(float);
			b->timestamp = r->ts_start;
			b->frames = obs_encoder_pause_audio_left(b->frames, cut);
			r->ts_start = 0;
			r->ts_end = 0;
			return obs_encoder_pause_audio_drop(b->frames);
		}
		return true;
	}
	return false;
}

static bool audio(struct record *r, struct buffer *b)
{
	bool drop = audio_internal(r, b);
	b->timestamp -= r->ts_offset;
	return drop;
}

static void check_record(const struct record *r, uint64_t ts_start, uint64_t ts_end)
{
	assert_int_equal(r->ts_start, ts_start);
	assert_int_equal(r->ts_end, ts_end);
}

static void check_buffer(const struct buffer *b, uint32_t frames, uint64_t timestamp, size_t advanced)
{
	assert_int_equal(b->frames, frames);
	assert_int_equal(b->timestamp, timestamp);
	assert_ptr_equal(b->data[0], (uint8_t *)plane0 + advanced);
	assert_ptr_equal(b->data[1], (uint8_t *)plane1 + advanced);
}

/* ------------------------------------------------------------------------ */
/* decisions */

static void a_pause_is_off_only_at_zero(void **state)
{
	(void)state;
	assert_true(obs_encoder_pause_off(0));
	assert_false(obs_encoder_pause_off(1));
	assert_false(obs_encoder_pause_off(T0));
	assert_false(obs_encoder_pause_off(UINT64_MAX));
}

static void video_steps(void **state)
{
	(void)state;
	const uint64_t s = 2 * T0, e = 3 * T0;

	/* exactly at the end: ends the pause */
	assert_true(obs_encoder_pause_video_end(e, e));
	/* an unpause in the same frame as the pause */
	assert_true(obs_encoder_pause_video_end(s, s));
	/* before the end, or with no end yet: does not */
	assert_false(obs_encoder_pause_video_end(e, s));
	assert_false(obs_encoder_pause_video_end(e, e - 1));
	assert_false(obs_encoder_pause_video_end(0, s));
	assert_false(obs_encoder_pause_video_end(0, UINT64_MAX));
	/* a frame stamped 0 while the end is not set ends the pause
	 * (characterized, not endorsed: frames are never stamped 0 here) */
	assert_true(obs_encoder_pause_video_end(0, 0));

	/* before the start: passes */
	assert_false(obs_encoder_pause_video_drop(s, s - 1));
	assert_false(obs_encoder_pause_video_drop(s, 1));
	/* at the start and after it: dropped */
	assert_true(obs_encoder_pause_video_drop(s, s));
	assert_true(obs_encoder_pause_video_drop(s, e - 1));
	assert_true(obs_encoder_pause_video_drop(s, UINT64_MAX));
}

static void audio_buffer_end(void **state)
{
	(void)state;
	assert_int_equal(obs_encoder_pause_audio_end(T0, BUFFER_NS), T0 + BUFFER_NS);
	assert_int_equal(obs_encoder_pause_audio_end(T0, 0), T0);
	/* a uint64_t sum: wraps (characterized, not endorsed) */
	assert_int_equal(obs_encoder_pause_audio_end(UINT64_MAX, 2), 1);
}

static void audio_start_against_the_buffer(void **state)
{
	(void)state;
	const uint64_t t = T0, e = T0 + BUFFER_NS;

	assert_int_equal(obs_encoder_pause_audio_start(e + 1, t, e), OBS_ENCODER_PAUSE_AUDIO_START_AFTER);
	assert_int_equal(obs_encoder_pause_audio_start(UINT64_MAX, t, e), OBS_ENCODER_PAUSE_AUDIO_START_AFTER);
	assert_int_equal(obs_encoder_pause_audio_start(t, t, e), OBS_ENCODER_PAUSE_AUDIO_START_INSIDE);
	assert_int_equal(obs_encoder_pause_audio_start(t + 1, t, e), OBS_ENCODER_PAUSE_AUDIO_START_INSIDE);
	assert_int_equal(obs_encoder_pause_audio_start(e, t, e), OBS_ENCODER_PAUSE_AUDIO_START_INSIDE);
	assert_int_equal(obs_encoder_pause_audio_start(t - 1, t, e), OBS_ENCODER_PAUSE_AUDIO_START_BEFORE);
	assert_int_equal(obs_encoder_pause_audio_start(1, t, e), OBS_ENCODER_PAUSE_AUDIO_START_BEFORE);
	/* an empty buffer holds only its start */
	assert_int_equal(obs_encoder_pause_audio_start(t, t, t), OBS_ENCODER_PAUSE_AUDIO_START_INSIDE);
	assert_int_equal(obs_encoder_pause_audio_start(t + 1, t, t), OBS_ENCODER_PAUSE_AUDIO_START_AFTER);
}

static void audio_end_against_the_buffer(void **state)
{
	(void)state;
	const uint64_t t = T0, e = T0 + BUFFER_NS;

	/* the end not set yet: dropped */
	assert_int_equal(obs_encoder_pause_audio_paused(0, t, e), OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP);
	/* the end inside the buffer, at either end of it: unpause */
	assert_int_equal(obs_encoder_pause_audio_paused(t, t, e), OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE);
	assert_int_equal(obs_encoder_pause_audio_paused(t + 1, t, e), OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE);
	assert_int_equal(obs_encoder_pause_audio_paused(e, t, e), OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE);
	/* the end after the buffer: dropped */
	assert_int_equal(obs_encoder_pause_audio_paused(e + 1, t, e), OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP);
	assert_int_equal(obs_encoder_pause_audio_paused(UINT64_MAX, t, e), OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP);
}

static void audio_cuts(void **state)
{
	(void)state;
	assert_int_equal(obs_encoder_pause_audio_cut_ns(T0 + 10000000, T0), 10000000);
	assert_int_equal(obs_encoder_pause_audio_cut_ns(T0, T0), 0);

	assert_int_equal(obs_encoder_pause_audio_kept(480), 480);
	assert_int_equal(obs_encoder_pause_audio_kept(0), 0);
	/* truncated to uint32_t (characterized, not endorsed: a cut is never
	 * longer than its buffer) */
	assert_int_equal(obs_encoder_pause_audio_kept(((uint64_t)1 << 32) + 5), 5);

	assert_int_equal(obs_encoder_pause_audio_left(FRAMES, 240), FRAMES - 240);
	assert_int_equal(obs_encoder_pause_audio_left(FRAMES, 0), FRAMES);
	assert_int_equal(obs_encoder_pause_audio_left(FRAMES, FRAMES), 0);
	assert_int_equal(obs_encoder_pause_audio_left(FRAMES, ((uint64_t)1 << 32) + 24), FRAMES - 24);
	/* a uint32_t difference: wraps (characterized, not endorsed) */
	assert_int_equal(obs_encoder_pause_audio_left(0, 1), UINT32_MAX);

	assert_true(obs_encoder_pause_audio_drop(0));
	assert_false(obs_encoder_pause_audio_drop(1));
	assert_false(obs_encoder_pause_audio_drop(FRAMES));
}

/* ------------------------------------------------------------------------ */
/* the checks */

static void video_through_a_pause(void **state)
{
	(void)state;
	struct record r = {0};
	const uint64_t frame = 16666667;
	const uint64_t s = T0 + 3 * frame, e = T0 + 9 * frame;

	/* no pause: every frame passes, and its time is kept */
	assert_false(video(&r, T0));
	assert_int_equal(r.last_video_ts, T0);

	r.ts_start = s;
	assert_false(video(&r, s - frame));
	assert_true(video(&r, s));
	assert_true(video(&r, s + frame));
	check_record(&r, s, 0);

	end(&r, e);
	assert_true(video(&r, e - frame));
	check_record(&r, s, e);
	assert_false(video(&r, e));
	check_record(&r, 0, 0);
	assert_int_equal(r.last_video_ts, e);
	assert_false(video(&r, e + frame));
	assert_int_equal(r.ts_offset, e - s);
}

static void audio_through_a_pause(void **state)
{
	(void)state;
	struct record r = {0};
	struct buffer b;
	const uint64_t s = T0 + 10000000;
	const uint64_t b2 = T0 + BUFFER_NS, b3 = T0 + 2 * BUFFER_NS, b4 = T0 + 3 * BUFFER_NS;
	const uint64_t e = b3 + 5000000;

	/* no pause: passes as it is */
	fill(&b, FRAMES, T0 - BUFFER_NS);
	assert_false(audio(&r, &b));
	check_buffer(&b, FRAMES, T0 - BUFFER_NS, 0);

	/* the pause starts 10 ms in: 480 frames kept */
	r.ts_start = s;
	fill(&b, FRAMES, T0);
	assert_false(audio(&r, &b));
	check_buffer(&b, 480, T0, 0);
	check_record(&r, s, 0);

	/* wholly inside the pause, before and after its end is set: dropped */
	fill(&b, FRAMES, b2);
	assert_true(audio(&r, &b));
	check_record(&r, s, 0);
	end(&r, e);
	fill(&b, FRAMES, b2);
	assert_true(audio(&r, &b));
	check_record(&r, s, e);

	/* the pause ends 5 ms in: 240 frames cut from the front, the rest
	 * passes stamped with the pause start, then the whole offset is taken
	 * off (characterized, not endorsed: one pause length earlier than the
	 * audio before the pause continues) */
	fill(&b, FRAMES, b3);
	assert_false(audio(&r, &b));
	check_buffer(&b, FRAMES - 240, s - (e - s), 240 * sizeof(float));
	check_record(&r, 0, 0);

	/* after the pause: passes, with the offset taken off */
	fill(&b, FRAMES, b4);
	assert_false(audio(&r, &b));
	check_buffer(&b, FRAMES, b4 - (e - s), 0);
}

static void a_start_at_either_end_of_a_buffer(void **state)
{
	(void)state;
	struct record r = {0};
	struct buffer b;

	/* at its start: nothing kept, dropped */
	r.ts_start = T0;
	fill(&b, FRAMES, T0);
	assert_true(audio(&r, &b));
	check_buffer(&b, 0, T0, 0);
	check_record(&r, T0, 0);

	/* at its end: 1023 of 1024 frames kept, as both conversions truncate
	 * (characterized, not endorsed) */
	r.ts_start = T0 + BUFFER_NS;
	fill(&b, FRAMES, T0);
	assert_false(audio(&r, &b));
	check_buffer(&b, FRAMES - 1, T0, 0);
	check_record(&r, T0 + BUFFER_NS, 0);

	/* an empty buffer at the start: still empty, dropped */
	r.ts_start = T0;
	fill(&b, 0, T0);
	assert_true(audio(&r, &b));
	check_buffer(&b, 0, T0, 0);
}

static void an_end_at_either_end_of_a_buffer(void **state)
{
	(void)state;
	struct record r = {0};
	struct buffer b;
	const uint64_t s = T0 - 3 * BUFFER_NS;

	/* at its start: nothing cut, the whole buffer passes, stamped with the
	 * pause start before the offset */
	r.ts_start = s;
	end(&r, T0);
	fill(&b, FRAMES, T0);
	assert_false(audio(&r, &b));
	check_buffer(&b, FRAMES, s - (T0 - s), 0);
	check_record(&r, 0, 0);

	/* at its end: 1023 frames cut, one passes */
	r.ts_start = s;
	r.ts_offset = 0;
	end(&r, T0 + BUFFER_NS);
	fill(&b, FRAMES, T0);
	assert_false(audio(&r, &b));
	check_buffer(&b, 1, s - (T0 + BUFFER_NS - s), (FRAMES - 1) * sizeof(float));
	check_record(&r, 0, 0);
}

static void the_offset_is_taken_off_every_buffer(void **state)
{
	(void)state;
	struct record r = {.ts_offset = 5000};
	struct buffer b;

	/* passed with no pause on */
	fill(&b, FRAMES, T0);
	assert_false(audio(&r, &b));
	check_buffer(&b, FRAMES, T0 - 5000, 0);

	/* dropped inside a pause: the offset is taken off it too */
	r.ts_start = T0 - 1;
	fill(&b, FRAMES, T0);
	assert_true(audio(&r, &b));
	check_buffer(&b, FRAMES, T0 - 5000, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(a_pause_is_off_only_at_zero),
		cmocka_unit_test(video_steps),
		cmocka_unit_test(audio_buffer_end),
		cmocka_unit_test(audio_start_against_the_buffer),
		cmocka_unit_test(audio_end_against_the_buffer),
		cmocka_unit_test(audio_cuts),
		cmocka_unit_test(video_through_a_pause),
		cmocka_unit_test(audio_through_a_pause),
		cmocka_unit_test(a_start_at_either_end_of_a_buffer),
		cmocka_unit_test(an_end_at_either_end_of_a_buffer),
		cmocka_unit_test(the_offset_is_taken_off_every_buffer),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
