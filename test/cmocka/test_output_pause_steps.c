#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-pause.h"

/*
 * Characterization of the pause plan of libobs/obs-output.c, moved to
 * libobs/obs-output-pause.c before its Rust port (#3020):
 * - obs_output_pause(): obs_output_pause_allowed(), the OBS_OUTPUT_CAN_PAUSE
 *   gate, and obs_output_pause_already(), the "already in that state"
 *   success;
 * - obs_encoded_output_pause(): obs_output_pause_lock_set(), the slots whose
 *   encoder's pause record is locked, checked and updated;
 * - get_closest_v_ts(): obs_output_pause_closest_ts(), the pause time on the
 *   video frame grid;
 * - pause_can_start(), pause_can_stop() and end_pause():
 *   obs_output_pause_started() and _ended(), one test per field (joined
 *   below with ! and && as obs-output.c joins them), and _offset().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. The scenarios run them
 * the way obs-output.c does, on stand-in records (encoded_pause() and
 * raw_pause() below follow obs_encoded_output_pause() and
 * obs_raw_output_pause() step by step); obs-output.c keeps the locks, the
 * atomics, the clock, the signals and the log.
 *
 * Not covered: one encoder in two slots (obs_encoded_output_pause() then
 * locks its pause mutex twice and hangs) and a frame interval of 0 (the grid
 * divides by it). The Rust port changes those on purpose; its parity test
 * lists them.
 */

/* 60 fps: video_frame_interval_ns for 60/1. */
#define FRAME 16666666ULL

#define NUM_VIDEO OBS_OUTPUT_PAUSE_MAX_VIDEO_ENCODERS
#define NUM_AUDIO OBS_OUTPUT_PAUSE_MAX_AUDIO_ENCODERS

/* ------------------------------------------------------------------------ */
/* stand-ins */

/* An encoder's (or a raw output's) struct pause_data and paused flag, and
 * how often its pause mutex was taken and given back. */
struct rec {
	uint64_t last_video_ts;
	uint64_t ts_start;
	uint64_t ts_end;
	uint64_t ts_offset;
	bool paused;
	int locks;
	int unlocks;
};

/* pause_can_start() and pause_can_stop(): the end is tested only once the
 * start test passed. */
static bool can_start(uint64_t ts_start, uint64_t ts_end)
{
	return !obs_output_pause_started(ts_start) && !obs_output_pause_ended(ts_end);
}

static bool can_stop(uint64_t ts_start, uint64_t ts_end)
{
	return obs_output_pause_started(ts_start) && !obs_output_pause_ended(ts_end);
}

static void end_pause(struct rec *r, uint64_t ts)
{
	if (!obs_output_pause_ended(r->ts_end)) {
		r->ts_end = ts;
		r->ts_offset = obs_output_pause_offset(r->ts_offset, r->ts_start, r->ts_end);
	}
}

static bool marked(uint32_t mask, size_t i)
{
	return (mask & ((uint32_t)1 << i)) != 0;
}

/* obs_encoded_output_pause(), on stand-in records, at clock now. */
static bool encoded_pause(struct rec *venc[NUM_VIDEO], struct rec *aenc[NUM_AUDIO], uint64_t now, bool pause)
{
	uintptr_t vid[NUM_VIDEO];
	uintptr_t aid[NUM_AUDIO];
	uint32_t vlock;
	uint32_t alock;
	bool success = false;

	for (size_t i = 0; i < NUM_VIDEO; i++)
		vid[i] = (uintptr_t)venc[i];
	for (size_t i = 0; i < NUM_AUDIO; i++)
		aid[i] = (uintptr_t)aenc[i];
	obs_output_pause_lock_set(vid, aid, &vlock, &alock);

	for (size_t i = 0; i < NUM_VIDEO; i++)
		if (marked(vlock, i))
			venc[i]->locks++;
	for (size_t i = 0; i < NUM_AUDIO; i++)
		if (marked(alock, i))
			aenc[i]->locks++;

	/* get_first_video_encoder_index() */
	size_t first = NUM_VIDEO;
	for (size_t i = 0; i < NUM_VIDEO; i++) {
		if (venc[i]) {
			first = i;
			break;
		}
	}
	if (first == NUM_VIDEO)
		goto fail;

	uint64_t ts = obs_output_pause_closest_ts(venc[first]->last_video_ts, FRAME, now);

	for (size_t i = 0; i < NUM_VIDEO; i++)
		if (marked(vlock, i) && !(pause ? can_start(venc[i]->ts_start, venc[i]->ts_end)
						: can_stop(venc[i]->ts_start, venc[i]->ts_end)))
			goto fail;
	for (size_t i = 0; i < NUM_AUDIO; i++)
		if (marked(alock, i) && !(pause ? can_start(aenc[i]->ts_start, aenc[i]->ts_end)
						: can_stop(aenc[i]->ts_start, aenc[i]->ts_end)))
			goto fail;

	for (size_t i = 0; i < NUM_VIDEO + NUM_AUDIO; i++) {
		bool video = i < NUM_VIDEO;
		size_t slot = video ? i : i - NUM_VIDEO;
		if (!marked(video ? vlock : alock, slot))
			continue;
		struct rec *r = video ? venc[slot] : aenc[slot];
		r->paused = pause;
		if (pause)
			r->ts_start = ts;
		else
			end_pause(r, ts);
	}

	success = true;

fail:
	for (size_t i = NUM_AUDIO; i > 0; i--)
		if (marked(alock, i - 1))
			aenc[i - 1]->unlocks++;
	for (size_t i = NUM_VIDEO; i > 0; i--)
		if (marked(vlock, i - 1))
			venc[i - 1]->unlocks++;
	return success;
}

/* obs_raw_output_pause(), on the output's own stand-in record. */
static bool raw_pause(struct rec *r, uint64_t now, bool pause)
{
	uint64_t ts = obs_output_pause_closest_ts(r->last_video_ts, FRAME, now);
	bool success;

	if (pause) {
		success = can_start(r->ts_start, r->ts_end);
		if (success)
			r->ts_start = ts;
	} else {
		success = can_stop(r->ts_start, r->ts_end);
		if (success)
			end_pause(r, ts);
	}
	return success;
}

static void check_rec(const char *what, const struct rec *r, uint64_t start, uint64_t end, uint64_t offset)
{
	if (r->ts_start != start || r->ts_end != end || r->ts_offset != offset) {
		fprintf(stderr, "%s: start %llu end %llu offset %llu; expected %llu %llu %llu\n", what,
			(unsigned long long)r->ts_start, (unsigned long long)r->ts_end,
			(unsigned long long)r->ts_offset, (unsigned long long)start, (unsigned long long)end,
			(unsigned long long)offset);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* obs_output_pause() gates */

static void only_an_output_that_can_pause(void **state)
{
	(void)state;
	assert_true(obs_output_pause_allowed(OBS_OUTPUT_PAUSE_CAN_PAUSE));
	assert_true(obs_output_pause_allowed(0xFFFFFFFFu));
	/* video, audio, encoded and service, with and without it */
	assert_true(obs_output_pause_allowed(0x0Fu | OBS_OUTPUT_PAUSE_CAN_PAUSE));
	assert_false(obs_output_pause_allowed(0x0Fu));
	assert_false(obs_output_pause_allowed(0));
	assert_false(obs_output_pause_allowed(0xFFFFFFFFu & ~(uint32_t)OBS_OUTPUT_PAUSE_CAN_PAUSE));
}

static void already_in_that_state_succeeds(void **state)
{
	(void)state;
	/* Pausing a paused output, or unpausing a running one, returns true
	 * with nothing done. */
	assert_true(obs_output_pause_already(true, true));
	assert_true(obs_output_pause_already(false, false));
	assert_false(obs_output_pause_already(false, true));
	assert_false(obs_output_pause_already(true, false));
}

/* ------------------------------------------------------------------------ */
/* the lock set */

static void every_slot_with_an_encoder_is_locked(void **state)
{
	(void)state;
	uintptr_t video[NUM_VIDEO] = {0};
	uintptr_t audio[NUM_AUDIO] = {0};
	uint32_t vlock = 0xdead;
	uint32_t alock = 0xbeef;

	obs_output_pause_lock_set(video, audio, &vlock, &alock);
	assert_int_equal(vlock, 0);
	assert_int_equal(alock, 0);

	/* one video encoder, two audio tracks (the recording's usual set) */
	video[0] = 0x1000;
	audio[0] = 0x2000;
	audio[1] = 0x3000;
	obs_output_pause_lock_set(video, audio, &vlock, &alock);
	assert_int_equal(vlock, 0x1);
	assert_int_equal(alock, 0x3);

	/* gaps stay unmarked */
	video[0] = 0;
	video[3] = 0x1000;
	video[9] = 0x1100;
	audio[1] = 0;
	audio[5] = 0x3000;
	obs_output_pause_lock_set(video, audio, &vlock, &alock);
	assert_int_equal(vlock, (1u << 3) | (1u << 9));
	assert_int_equal(alock, (1u << 0) | (1u << 5));

	/* every slot, each a different encoder */
	for (size_t i = 0; i < NUM_VIDEO; i++)
		video[i] = 0x1000 + i;
	for (size_t i = 0; i < NUM_AUDIO; i++)
		audio[i] = 0x2000 + i;
	obs_output_pause_lock_set(video, audio, &vlock, &alock);
	assert_int_equal(vlock, 0x3FF);
	assert_int_equal(alock, 0x3F);
}

/* ------------------------------------------------------------------------ */
/* the pause time */

static void the_pause_time_snaps_to_the_frame_grid(void **state)
{
	(void)state;
	const uint64_t last = 5000000000ULL;

	/* no frame yet: two frames on from 0 */
	assert_true(obs_output_pause_closest_ts(0, FRAME, 0) == 2 * FRAME);
	assert_true(obs_output_pause_closest_ts(0, FRAME, FRAME - 1) == 2 * FRAME);
	assert_true(obs_output_pause_closest_ts(0, FRAME, FRAME) == 3 * FRAME);

	/* a clock on the grid: two frames after it */
	assert_true(obs_output_pause_closest_ts(last, FRAME, last) == last + 2 * FRAME);
	assert_true(obs_output_pause_closest_ts(last, FRAME, last + 5 * FRAME) == last + 7 * FRAME);

	/* one ns past a frame, or one before the next: truncated to the
	 * frame, then two on */
	assert_true(obs_output_pause_closest_ts(last, FRAME, last + 5 * FRAME + 1) == last + 7 * FRAME);
	assert_true(obs_output_pause_closest_ts(last, FRAME, last + 6 * FRAME - 1) == last + 7 * FRAME);

	/* a clock a frame behind the last frame: the frame after it */
	assert_true(obs_output_pause_closest_ts(last, FRAME, last - FRAME) == last + FRAME);

	/* other rates: 30 fps and 1 ns frames */
	assert_true(obs_output_pause_closest_ts(last, 33333333ULL, last + 100000000ULL) == last + 5 * 33333333ULL);
	assert_true(obs_output_pause_closest_ts(7, 1, 10) == 12);
}

/* ------------------------------------------------------------------------ */
/* the records */

static void a_record_starts_and_ends_a_pause(void **state)
{
	(void)state;
	/* clear: can start, not stop */
	assert_true(can_start(0, 0));
	assert_false(can_stop(0, 0));
	/* started: can stop, not start */
	assert_false(can_start(10, 0));
	assert_true(can_stop(10, 0));
	/* ended (not cleared yet): neither */
	assert_false(can_start(10, 20));
	assert_false(can_stop(10, 20));
	/* an end with no start: neither (characterized, not endorsed: never
	 * set that way) */
	assert_false(can_start(0, 20));
	assert_false(can_stop(0, 20));

	/* each field on its own; the end is set once */
	assert_false(obs_output_pause_started(0));
	assert_true(obs_output_pause_started(1));
	assert_true(obs_output_pause_started(UINT64_MAX));
	assert_false(obs_output_pause_ended(0));
	assert_true(obs_output_pause_ended(1));
	assert_true(obs_output_pause_ended(UINT64_MAX));

	/* the offset grows by the pause's length */
	assert_true(obs_output_pause_offset(0, 100, 350) == 250);
	assert_true(obs_output_pause_offset(250, 1000, 1500) == 750);
	assert_true(obs_output_pause_offset(7, 5, 5) == 7);
}

/* ------------------------------------------------------------------------ */
/* scenarios */

static void pause_and_unpause_one_video_two_audio(void **state)
{
	(void)state;
	struct rec v = {.last_video_ts = 1000 * FRAME};
	struct rec a1 = {.last_video_ts = 77};
	struct rec a2 = {0};
	struct rec *venc[NUM_VIDEO] = {&v};
	struct rec *aenc[NUM_AUDIO] = {&a1, &a2};

	/* Every record gets the first video encoder's grid time. */
	uint64_t start = 1000 * FRAME + 7 * FRAME;
	assert_true(encoded_pause(venc, aenc, 1000 * FRAME + 5 * FRAME + 3, true));
	check_rec("video, paused", &v, start, 0, 0);
	check_rec("audio 1, paused", &a1, start, 0, 0);
	check_rec("audio 2, paused", &a2, start, 0, 0);
	assert_true(v.paused && a1.paused && a2.paused);

	/* The video encoder moved on while paused. */
	v.last_video_ts = 1100 * FRAME;
	uint64_t end = 1100 * FRAME + 2 * FRAME;
	assert_true(encoded_pause(venc, aenc, 1100 * FRAME, false));
	check_rec("video, unpaused", &v, start, end, end - start);
	check_rec("audio 1, unpaused", &a1, start, end, end - start);
	check_rec("audio 2, unpaused", &a2, start, end, end - start);
	assert_false(v.paused || a1.paused || a2.paused);

	/* Each mutex taken and given back once per call. */
	assert_int_equal(v.locks, 2);
	assert_int_equal(v.unlocks, 2);
	assert_int_equal(a1.locks, 2);
	assert_int_equal(a2.unlocks, 2);

	/* Until the encoders clear their records, a second pause fails. */
	assert_false(encoded_pause(venc, aenc, 1200 * FRAME, true));
	check_rec("video, again", &v, start, end, end - start);

	/* Cleared with the offset kept (as the encoders do): the next pause
	 * adds to it. */
	v.ts_start = v.ts_end = a1.ts_start = a1.ts_end = a2.ts_start = a2.ts_end = 0;
	v.last_video_ts = 1200 * FRAME;
	assert_true(encoded_pause(venc, aenc, 1200 * FRAME, true));
	assert_true(encoded_pause(venc, aenc, 1300 * FRAME, false));
	check_rec("video, second", &v, 1202 * FRAME, 1302 * FRAME, end - start + 100 * FRAME);
}

static void one_record_not_clear_fails_the_whole_pause(void **state)
{
	(void)state;
	struct rec v = {.last_video_ts = 10 * FRAME};
	struct rec a1 = {0};
	struct rec a2 = {.ts_start = 3 * FRAME, .ts_end = 4 * FRAME, .ts_offset = FRAME};
	struct rec *venc[NUM_VIDEO] = {&v};
	struct rec *aenc[NUM_AUDIO] = {&a1, &a2};

	assert_false(encoded_pause(venc, aenc, 12 * FRAME, true));
	check_rec("video", &v, 0, 0, 0);
	check_rec("audio 1", &a1, 0, 0, 0);
	check_rec("audio 2", &a2, 3 * FRAME, 4 * FRAME, FRAME);
	assert_false(v.paused || a1.paused || a2.paused);
	/* every lock still given back */
	assert_int_equal(v.unlocks, 1);
	assert_int_equal(a2.unlocks, 1);

	/* unpause with one record never started: fails too */
	a2 = (struct rec){0};
	v.ts_start = a1.ts_start = 5 * FRAME;
	assert_false(encoded_pause(venc, aenc, 12 * FRAME, false));
	check_rec("video, unpause", &v, 5 * FRAME, 0, 0);
}

static void audio_encoders_only_never_pause(void **state)
{
	(void)state;
	/* Characterized, not endorsed: with no video encoder there is no clock
	 * to snap to, so an encoded audio-only output can't pause. */
	struct rec a1 = {0};
	struct rec *venc[NUM_VIDEO] = {0};
	struct rec *aenc[NUM_AUDIO] = {&a1};

	assert_false(encoded_pause(venc, aenc, 12 * FRAME, true));
	check_rec("audio", &a1, 0, 0, 0);
	assert_int_equal(a1.locks, 1);
	assert_int_equal(a1.unlocks, 1);
}

static void a_raw_output_pauses_on_its_own_record(void **state)
{
	(void)state;
	struct rec r = {.last_video_ts = 50 * FRAME, .ts_offset = 3};

	assert_false(raw_pause(&r, 51 * FRAME, false));
	assert_true(raw_pause(&r, 51 * FRAME, true));
	check_rec("paused", &r, 53 * FRAME, 0, 3);
	assert_false(raw_pause(&r, 52 * FRAME, true));
	assert_true(raw_pause(&r, 60 * FRAME, false));
	check_rec("unpaused", &r, 53 * FRAME, 62 * FRAME, 3 + 9 * FRAME);
	assert_false(raw_pause(&r, 70 * FRAME, false));
	check_rec("unpaused twice", &r, 53 * FRAME, 62 * FRAME, 3 + 9 * FRAME);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(only_an_output_that_can_pause),
		cmocka_unit_test(already_in_that_state_succeeds),
		cmocka_unit_test(every_slot_with_an_encoder_is_locked),
		cmocka_unit_test(the_pause_time_snaps_to_the_frame_grid),
		cmocka_unit_test(a_record_starts_and_ends_a_pause),
		cmocka_unit_test(pause_and_unpause_one_video_two_audio),
		cmocka_unit_test(one_record_not_clear_fails_the_whole_pause),
		cmocka_unit_test(audio_encoders_only_never_pause),
		cmocka_unit_test(a_raw_output_pauses_on_its_own_record),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
