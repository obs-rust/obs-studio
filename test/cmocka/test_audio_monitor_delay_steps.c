#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "audio-monitoring/monitor-delay.h"
#include "audio-monitoring/monitor-video-sync.h"

/* Characterization of the WASAPI monitor's delay decisions, moved from
 * process_audio_delay() in libobs/audio-monitoring/win32/wasapi-output.c to
 * libobs/audio-monitoring/monitor-delay.c: the stale cut and the verdict on
 * the packet at the front of the delay queue. The file includes no Windows
 * header, so this runs on every platform; the WASAPI monitor itself only
 * runs on Windows. The functions are hidden in libobs, so this
 * characterizes that C source; the test calls only them. */

#define MS 1000000ULL
#define S 1000000000ULL
/* a video timestamp well away from 0 and from the limits */
#define VIDEO (10 * S)

static void expect_verdict(enum audio_monitor_delay_verdict got, enum audio_monitor_delay_verdict want,
			   const char *what, long long arg)
{
	if (got != want) {
		fprintf(stderr, "%s (%lld): verdict %d, expected %d\n", what, arg, (int)got, (int)want);
		fail();
	}
}

/* Times judge() asked for the stall count. */
static int stall_asks;

/* The front packet's verdict, its steps asked as process_audio_delay() asks
 * them: the difference, then the gate, whose second part (the stall count)
 * only when the first does not decide it, then the verdict. */
static enum audio_monitor_delay_verdict judge(uint64_t cur_ts, uint64_t pad_ns, uint64_t last_frame_ts,
					      uint64_t time_since_prev, uint64_t *front_ts, int64_t *diff)
{
	bool bad_diff;

	*diff = audio_monitor_delay_diff(cur_ts, pad_ns, last_frame_ts, front_ts);
	bad_diff = audio_monitor_delay_unsynced(last_frame_ts, *diff) ||
		   (stall_asks++, audio_monitor_delay_stalled(time_since_prev));
	return audio_monitor_delay_judge_diff(bad_diff, *diff);
}

/* The verdict on a front packet whose timestamp less the padding is diff
 * away from VIDEO, with the video moving; checks what is written back. */
static enum audio_monitor_delay_verdict judge_at(int64_t diff)
{
	uint64_t front_ts = 0;
	int64_t got_diff = 0;
	enum audio_monitor_delay_verdict v = judge(VIDEO + (uint64_t)diff, 0, VIDEO, 0, &front_ts, &got_diff);
	assert_int_equal(front_ts, VIDEO + (uint64_t)diff);
	assert_int_equal(got_diff, diff);
	return v;
}

static void constants(void **state)
{
	(void)state;
	assert_int_equal(AUDIO_MONITOR_DELAY_STALE_NS, 1000000000);
	assert_int_equal(AUDIO_MONITOR_DELAY_MAX_DIFF_NS, 5000000000LL);
	assert_int_equal(AUDIO_MONITOR_DELAY_SYNC_NS, 75000000);
	assert_int_equal(AUDIO_MONITOR_VIDEO_STALL_NS, 100000000ULL);
	assert_int_equal(AUDIO_MONITOR_DELAY_PLAY, 0);
	assert_int_equal(AUDIO_MONITOR_DELAY_HOLD, 1);
	assert_int_equal(AUDIO_MONITOR_DELAY_DRAG, 2);
}

/* More than 1 s since the last packet: stale. The time is stored every
 * time. */
static void stale_cut(void **state)
{
	(void)state;
	uint64_t last = 5 * S;

	assert_false(audio_monitor_delay_stale(&last, 5 * S));
	assert_int_equal(last, 5 * S);
	assert_false(audio_monitor_delay_stale(&last, 5 * S + 1));
	assert_int_equal(last, 5 * S + 1);
	assert_false(audio_monitor_delay_stale(&last, 6 * S + 1));
	assert_int_equal(last, 6 * S + 1);
	assert_true(audio_monitor_delay_stale(&last, 7 * S + 2));
	assert_int_equal(last, 7 * S + 2);
	assert_false(audio_monitor_delay_stale(&last, 7 * S + 2 + 20 * MS));
	assert_int_equal(last, 7 * S + 2 + 20 * MS);

	/* the first packet: the receive time starts at 0 */
	last = 0;
	assert_false(audio_monitor_delay_stale(&last, S));
	assert_int_equal(last, S);
	last = 0;
	assert_true(audio_monitor_delay_stale(&last, S + 1));
	last = 0;
	assert_false(audio_monitor_delay_stale(&last, 0));

	/* Characterized, not endorsed: a clock that steps back wraps the
	 * uint64_t difference, so the queue is stale. */
	last = 5 * S;
	assert_true(audio_monitor_delay_stale(&last, 5 * S - 1));
	assert_int_equal(last, 5 * S - 1);

	/* the far ends of the clock */
	last = 0;
	assert_true(audio_monitor_delay_stale(&last, UINT64_MAX));
	assert_int_equal(last, UINT64_MAX);
	assert_false(audio_monitor_delay_stale(&last, UINT64_MAX));
	assert_true(audio_monitor_delay_stale(&last, S));
}

/* Each step on its own: the difference (wrapping as the C's uint64_t
 * subtraction and casts do), the two parts of the gate and the verdict. */
static void steps_alone(void **state)
{
	(void)state;
	uint64_t front_ts = 0;

	assert_int_equal(audio_monitor_delay_diff(VIDEO + S, 0, VIDEO, &front_ts), S);
	assert_int_equal(front_ts, VIDEO + S);
	assert_int_equal(audio_monitor_delay_diff(VIDEO, 100 * MS, VIDEO, &front_ts), -100 * (int64_t)MS);
	assert_int_equal(front_ts, VIDEO - 100 * MS);
	assert_int_equal(audio_monitor_delay_diff(0, 1, 0, &front_ts), -1);
	assert_int_equal(front_ts, UINT64_MAX);

	/* no video yet, or more than 5 s apart */
	assert_true(audio_monitor_delay_unsynced(0, 0));
	assert_true(audio_monitor_delay_unsynced(0, 75 * (int64_t)MS + 1));
	assert_false(audio_monitor_delay_unsynced(1, 0));
	assert_false(audio_monitor_delay_unsynced(VIDEO, 5 * (int64_t)S));
	assert_false(audio_monitor_delay_unsynced(VIDEO, -5 * (int64_t)S));
	assert_true(audio_monitor_delay_unsynced(VIDEO, 5 * (int64_t)S + 1));
	assert_true(audio_monitor_delay_unsynced(VIDEO, -5 * (int64_t)S - 1));
	assert_true(audio_monitor_delay_unsynced(VIDEO, INT64_MAX));
	assert_true(audio_monitor_delay_unsynced(VIDEO, INT64_MIN + 1));

	/* the video stood still for more than 100 ms */
	assert_false(audio_monitor_delay_stalled(0));
	assert_false(audio_monitor_delay_stalled(100 * MS - 1));
	assert_false(audio_monitor_delay_stalled(100 * MS));
	assert_true(audio_monitor_delay_stalled(100 * MS + 1));
	assert_true(audio_monitor_delay_stalled(UINT64_MAX));

	/* the verdict: only a good difference holds or drags */
	expect_verdict(audio_monitor_delay_judge_diff(false, 75 * (int64_t)MS + 1), AUDIO_MONITOR_DELAY_HOLD, "good",
		       1);
	expect_verdict(audio_monitor_delay_judge_diff(false, 75 * (int64_t)MS), AUDIO_MONITOR_DELAY_PLAY, "good", 0);
	expect_verdict(audio_monitor_delay_judge_diff(false, -75 * (int64_t)MS), AUDIO_MONITOR_DELAY_PLAY, "good", 0);
	expect_verdict(audio_monitor_delay_judge_diff(false, -75 * (int64_t)MS - 1), AUDIO_MONITOR_DELAY_DRAG, "good",
		       -1);
	expect_verdict(audio_monitor_delay_judge_diff(false, INT64_MAX), AUDIO_MONITOR_DELAY_HOLD, "good", 2);
	expect_verdict(audio_monitor_delay_judge_diff(false, INT64_MIN), AUDIO_MONITOR_DELAY_DRAG, "good", -2);
	expect_verdict(audio_monitor_delay_judge_diff(true, 75 * (int64_t)MS + 1), AUDIO_MONITOR_DELAY_PLAY, "bad", 1);
	expect_verdict(audio_monitor_delay_judge_diff(true, -75 * (int64_t)MS - 1), AUDIO_MONITOR_DELAY_PLAY, "bad",
		       -1);
	expect_verdict(audio_monitor_delay_judge_diff(true, 0), AUDIO_MONITOR_DELAY_PLAY, "bad", 0);
}

/* The stall count is asked for only when the first part of the gate does
 * not decide it: with a video timestamp and within 5 s. */
static void stall_asked_only_when_needed(void **state)
{
	(void)state;
	uint64_t front_ts;
	int64_t diff;

	stall_asks = 0;
	judge(VIDEO, 0, 0, 0, &front_ts, &diff);
	judge(VIDEO + 5 * S + 1, 0, VIDEO, 0, &front_ts, &diff);
	judge(VIDEO - 5 * S - 1, 0, VIDEO, 0, &front_ts, &diff);
	assert_int_equal(stall_asks, 0);
	judge(VIDEO + 5 * S, 0, VIDEO, 0, &front_ts, &diff);
	judge(VIDEO, 0, VIDEO, 200 * MS, &front_ts, &diff);
	assert_int_equal(stall_asks, 2);
}

/* Rushing (more than 75 ms ahead) holds, dragging (more than 75 ms behind)
 * drags, both only within 5 s; everything else plays. */
static void judge_thresholds(void **state)
{
	(void)state;
	static const struct {
		int64_t diff;
		enum audio_monitor_delay_verdict want;
	} cases[] = {
		{0, AUDIO_MONITOR_DELAY_PLAY},
		{1, AUDIO_MONITOR_DELAY_PLAY},
		{-1, AUDIO_MONITOR_DELAY_PLAY},
		{75 * (int64_t)MS - 1, AUDIO_MONITOR_DELAY_PLAY},
		{75 * (int64_t)MS, AUDIO_MONITOR_DELAY_PLAY},
		{75 * (int64_t)MS + 1, AUDIO_MONITOR_DELAY_HOLD},
		{-75 * (int64_t)MS + 1, AUDIO_MONITOR_DELAY_PLAY},
		{-75 * (int64_t)MS, AUDIO_MONITOR_DELAY_PLAY},
		{-75 * (int64_t)MS - 1, AUDIO_MONITOR_DELAY_DRAG},
		{S, AUDIO_MONITOR_DELAY_HOLD},
		{-(int64_t)S, AUDIO_MONITOR_DELAY_DRAG},
		{5 * (int64_t)S - 1, AUDIO_MONITOR_DELAY_HOLD},
		{5 * (int64_t)S, AUDIO_MONITOR_DELAY_HOLD},
		{5 * (int64_t)S + 1, AUDIO_MONITOR_DELAY_PLAY},
		{-5 * (int64_t)S + 1, AUDIO_MONITOR_DELAY_DRAG},
		{-5 * (int64_t)S, AUDIO_MONITOR_DELAY_DRAG},
		{-5 * (int64_t)S - 1, AUDIO_MONITOR_DELAY_PLAY},
		{-(int64_t)VIDEO + 1, AUDIO_MONITOR_DELAY_PLAY},
		{60 * (int64_t)S, AUDIO_MONITOR_DELAY_PLAY},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++)
		expect_verdict(judge_at(cases[i].diff), cases[i].want, "difference", (long long)cases[i].diff);
}

/* No video frame yet (last_frame_ts 0): the audio plays, whatever the
 * difference. */
static void judge_without_video(void **state)
{
	(void)state;
	static const uint64_t ts[] = {0, 1, 75 * MS + 1, S, 5 * S, 5 * S + 1, 60 * S};

	for (size_t i = 0; i < sizeof(ts) / sizeof(ts[0]); i++) {
		uint64_t front_ts = 0;
		int64_t diff = 0;
		expect_verdict(judge(ts[i], 0, 0, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY, "no video",
			       (long long)ts[i]);
		assert_int_equal(front_ts, ts[i]);
		assert_int_equal(diff, (int64_t)ts[i]);
	}
}

/* Video that has stood still for more than AUDIO_MONITOR_VIDEO_STALL_NS
 * (cover art): the audio plays; up to it, the audio still syncs. */
static void judge_stalled_video(void **state)
{
	(void)state;
	static const uint64_t stalls[] = {0, 1, 100 * MS - 1, 100 * MS};
	static const uint64_t stalled[] = {100 * MS + 1, 300 * MS, UINT64_MAX};
	uint64_t front_ts;
	int64_t diff;

	for (size_t i = 0; i < sizeof(stalls) / sizeof(stalls[0]); i++) {
		expect_verdict(judge(VIDEO + S, 0, VIDEO, stalls[i], &front_ts, &diff), AUDIO_MONITOR_DELAY_HOLD,
			       "syncing, ahead", (long long)stalls[i]);
		expect_verdict(judge(VIDEO - S, 0, VIDEO, stalls[i], &front_ts, &diff), AUDIO_MONITOR_DELAY_DRAG,
			       "syncing, behind", (long long)stalls[i]);
	}
	for (size_t i = 0; i < sizeof(stalled) / sizeof(stalled[0]); i++) {
		expect_verdict(judge(VIDEO + S, 0, VIDEO, stalled[i], &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY,
			       "stalled, ahead", (long long)(stalled[i] / MS));
		expect_verdict(judge(VIDEO - S, 0, VIDEO, stalled[i], &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY,
			       "stalled, behind", (long long)(stalled[i] / MS));
		assert_int_equal(front_ts, VIDEO - S);
		assert_int_equal(diff, -(int64_t)S);
	}
}

/* The device's padding comes off the packet's timestamp before it is
 * compared with the video. */
static void judge_padding(void **state)
{
	(void)state;
	uint64_t front_ts;
	int64_t diff;
	const uint64_t pad = 100 * MS;

	expect_verdict(judge(VIDEO + 75 * MS + 1 + pad, pad, VIDEO, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_HOLD,
		       "padding, ahead", 0);
	assert_int_equal(front_ts, VIDEO + 75 * MS + 1);
	assert_int_equal(diff, 75 * (int64_t)MS + 1);
	expect_verdict(judge(VIDEO + 75 * MS + pad, pad, VIDEO, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY,
		       "padding, at the edge", 0);
	expect_verdict(judge(VIDEO + pad, pad, VIDEO, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY,
		       "padding, in step", 0);
	expect_verdict(judge(VIDEO + 50 * MS, pad, VIDEO, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY,
		       "padding, behind within 75 ms", 0);
	assert_int_equal(diff, -50 * (int64_t)MS);
	expect_verdict(judge(VIDEO, pad, VIDEO, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_DRAG, "padding, behind", 0);
	assert_int_equal(front_ts, VIDEO - pad);
	assert_int_equal(diff, -(int64_t)pad);
}

/* Characterized, not endorsed: a padding longer than the packet's
 * timestamp wraps the uint64_t subtraction. Cast to int64_t, the front
 * timestamp is then negative: before the video, so the packet drags when
 * the video is near and plays when it is more than 5 s away. */
static void judge_padding_wraps(void **state)
{
	(void)state;
	uint64_t front_ts;
	int64_t diff;

	expect_verdict(judge(1000, 2000, 500 * MS, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_DRAG,
		       "wrapped, video near", 0);
	assert_int_equal(front_ts, UINT64_MAX - 999);
	assert_int_equal(diff, -1000 - 500 * (int64_t)MS);

	expect_verdict(judge(1000, 2000, VIDEO, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY, "wrapped, video far",
		       0);
	assert_int_equal(diff, -1000 - (int64_t)VIDEO);

	expect_verdict(judge(1000, 2000, 1, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY, "wrapped, video at 1 ns",
		       0);
	assert_int_equal(diff, -1001);
}

/* A packet timestamp past INT64_MAX is negative once cast: the same
 * comparison as a timestamp before 0. */
static void judge_large_timestamps(void **state)
{
	(void)state;
	uint64_t front_ts;
	int64_t diff;

	expect_verdict(judge(UINT64_MAX, 0, 1, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_PLAY, "timestamp UINT64_MAX",
		       0);
	assert_int_equal(diff, -2);
	expect_verdict(judge(UINT64_MAX - 100 * MS + 1, 0, 1, 0, &front_ts, &diff), AUDIO_MONITOR_DELAY_DRAG,
		       "timestamp 100 ms before 0", 0);
	expect_verdict(judge((uint64_t)INT64_MAX, 0, (uint64_t)INT64_MAX - S, 0, &front_ts, &diff),
		       AUDIO_MONITOR_DELAY_HOLD, "timestamp INT64_MAX", 0);
	assert_int_equal(diff, (int64_t)S);
}

/* process_audio_delay()'s loop over these, as wasapi-output.c asks them:
 * the front packet's verdict, a hold returns with the queue kept, a drag
 * drops the packet only while more are queued after it, and play returns
 * the packet. Each packet is its timestamp; returns the index played, or -1
 * for a hold, and leaves the first packet still queued in *head. */
static int stand_in_loop(const uint64_t *queue, size_t count, size_t *head, uint64_t pad_ns, uint64_t last_frame_ts,
			 uint64_t time_since_prev)
{
	while (*head < count) {
		uint64_t front_ts;
		int64_t diff;
		enum audio_monitor_delay_verdict verdict =
			judge(queue[*head], pad_ns, last_frame_ts, time_since_prev, &front_ts, &diff);

		if (verdict == AUDIO_MONITOR_DELAY_HOLD)
			return -1;

		size_t popped = (*head)++;

		if (verdict == AUDIO_MONITOR_DELAY_DRAG && *head < count)
			continue;

		return (int)popped;
	}
	return -2;
}

static void stand_in_queue(void **state)
{
	(void)state;
	/* 20 ms packets from 300 ms behind the video to 200 ms ahead */
	uint64_t queue[26];
	for (size_t i = 0; i < 26; i++)
		queue[i] = VIDEO - 300 * MS + i * 20 * MS;

	/* behind: the packets more than 75 ms behind are dropped, and the
	 * first one within it plays */
	size_t head = 0;
	assert_int_equal(stand_in_loop(queue, 26, &head, 0, VIDEO, 0), 12);
	assert_int_equal(head, 13);
	assert_int_equal(queue[12], VIDEO - 60 * MS);
	/* then each packet in turn, up to 75 ms ahead */
	for (int i = 13; i <= 18; i++)
		assert_int_equal(stand_in_loop(queue, 26, &head, 0, VIDEO, 0), i);
	/* 80 ms ahead: held, the queue kept */
	assert_int_equal(stand_in_loop(queue, 26, &head, 0, VIDEO, 0), -1);
	assert_int_equal(head, 19);
	/* the video moves on: it plays */
	assert_int_equal(stand_in_loop(queue, 26, &head, 0, VIDEO + 20 * MS, 0), 19);

	/* the last packet plays even when it drags */
	head = 0;
	assert_int_equal(stand_in_loop(queue, 1, &head, 0, VIDEO, 0), 0);
	assert_int_equal(head, 1);

	/* the padding moves every packet back: with 100 ms of it, 80 ms ahead
	 * is behind */
	head = 19;
	assert_int_equal(stand_in_loop(queue, 26, &head, 100 * MS, VIDEO, 0), 19);

	/* stalled video, or none: no packet is dropped or held */
	head = 0;
	assert_int_equal(stand_in_loop(queue, 26, &head, 0, VIDEO, 100 * MS + 1), 0);
	head = 25;
	assert_int_equal(stand_in_loop(queue, 26, &head, 0, VIDEO, 100 * MS + 1), 25);
	head = 0;
	assert_int_equal(stand_in_loop(queue, 26, &head, 0, 0, 0), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),           cmocka_unit_test(stale_cut),
		cmocka_unit_test(steps_alone),         cmocka_unit_test(stall_asked_only_when_needed),
		cmocka_unit_test(judge_thresholds),    cmocka_unit_test(judge_without_video),
		cmocka_unit_test(judge_stalled_video), cmocka_unit_test(judge_padding),
		cmocka_unit_test(judge_padding_wraps), cmocka_unit_test(judge_large_timestamps),
		cmocka_unit_test(stand_in_queue),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
