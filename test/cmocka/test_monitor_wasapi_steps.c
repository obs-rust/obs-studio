#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "audio-monitoring/monitor-speaker-layout.h"
#include "audio-monitoring/monitor-video-sync.h"

/* Characterization of the WASAPI monitor's speaker layout and video stall
 * tracker, moved from the static inline bodies of monitor-speaker-layout.h
 * and monitor-video-sync.h to monitor-speaker-layout.c and
 * monitor-video-sync.c: what test_monitor_speaker_layout and
 * test_monitor_video_sync leave open. The files include no Windows header,
 * so this runs on every platform; the WASAPI monitor itself only runs on
 * Windows. The functions are hidden in libobs, so this characterizes that C
 * source; the test calls only them. */

#define MS 1000000ULL
/* a 1024-frame packet at 48 kHz */
#define PACKET_NS (1024ULL * 1000000000ULL / 48000ULL)

struct mask_row {
	const char *name;
	uint32_t mask;
	/* the layout for 0 to 9 channels, as an int */
	int layout[10];
};

static const struct mask_row rows[] = {
	/* the five masks the monitor lists: their own layout for their own
	 * channel count, none for any other count */
	{"2.1", AUDIO_MONITOR_SPEAKER_MASK_2POINT1, {0, 0, 0, 3, 0, 0, 0, 0, 0, 0}},
	{"surround", AUDIO_MONITOR_SPEAKER_MASK_SURROUND, {0, 0, 0, 0, 4, 0, 0, 0, 0, 0}},
	{"4.1", AUDIO_MONITOR_SPEAKER_MASK_4POINT1, {0, 0, 0, 0, 0, 5, 0, 0, 0, 0}},
	{"5.1", AUDIO_MONITOR_SPEAKER_MASK_5POINT1, {0, 0, 0, 0, 0, 0, 6, 0, 0, 0}},
	{"7.1", AUDIO_MONITOR_SPEAKER_MASK_7POINT1, {0, 0, 0, 0, 0, 0, 0, 0, 8, 0}},
	/* any other mask: the layout numbered by the channel count, none for
	 * 0, 7 (no layout is numbered 7) or 9 */
	{"no mask", 0x0, {0, 1, 2, 3, 4, 5, 6, 0, 8, 0}},
	{"mono", 0x4, {0, 1, 2, 3, 4, 5, 6, 0, 8, 0}},
	{"stereo", 0x3, {0, 1, 2, 3, 4, 5, 6, 0, 8, 0}},
	{"quad", 0x33, {0, 1, 2, 3, 4, 5, 6, 0, 8, 0}},
	{"5.1 (side)", 0x60F, {0, 1, 2, 3, 4, 5, 6, 0, 8, 0}},
	{"6.1", 0x70F, {0, 1, 2, 3, 4, 5, 6, 0, 8, 0}},
	{"7.1 (surround)", 0x63F, {0, 1, 2, 3, 4, 5, 6, 0, 8, 0}},
	{"every bit", 0xFFFFFFFF, {0, 1, 2, 3, 4, 5, 6, 0, 8, 0}},
};

#define ROW_COUNT (sizeof(rows) / sizeof(rows[0]))

static void constants(void **state)
{
	(void)state;
	assert_int_equal(AUDIO_MONITOR_SPEAKER_MASK_2POINT1, 0x0B);
	assert_int_equal(AUDIO_MONITOR_SPEAKER_MASK_SURROUND, 0x107);
	assert_int_equal(AUDIO_MONITOR_SPEAKER_MASK_4POINT1, 0x10F);
	assert_int_equal(AUDIO_MONITOR_SPEAKER_MASK_5POINT1, 0x3F);
	assert_int_equal(AUDIO_MONITOR_SPEAKER_MASK_7POINT1, 0xFF);
	assert_int_equal(SPEAKERS_UNKNOWN, 0);
	assert_int_equal(SPEAKERS_MONO, 1);
	assert_int_equal(SPEAKERS_STEREO, 2);
	assert_int_equal(SPEAKERS_2POINT1, 3);
	assert_int_equal(SPEAKERS_4POINT0, 4);
	assert_int_equal(SPEAKERS_4POINT1, 5);
	assert_int_equal(SPEAKERS_5POINT1, 6);
	assert_int_equal(SPEAKERS_7POINT1, 8);
}

/* Every mask with 0 to 9 channels. A listed mask with another channel
 * count gives no layout, even where the count has one (5.1 with 2
 * channels), where an unlisted mask would give it. */
static void every_mask_and_count(void **state)
{
	(void)state;
	bool ok = true;

	for (size_t i = 0; i < ROW_COUNT; i++) {
		for (uint32_t channels = 0; channels < 10; channels++) {
			int got = (int)audio_monitor_speaker_layout(rows[i].mask, channels);
			if (got != rows[i].layout[channels]) {
				fprintf(stderr, "%s (mask 0x%x) with %u channels: layout %d, expected %d\n",
					rows[i].name, (unsigned)rows[i].mask, (unsigned)channels, got,
					rows[i].layout[channels]);
				ok = false;
			}
		}
	}

	if (!ok)
		fail();
}

/* 7 channels give layout value 7 in the default, which no layout has
 * (get_audio_channels() of it is 0): none, under every mask. */
static void seven_channels_have_no_layout(void **state)
{
	(void)state;
	for (size_t i = 0; i < ROW_COUNT; i++)
		assert_int_equal(audio_monitor_speaker_layout(rows[i].mask, 7), SPEAKERS_UNKNOWN);
	assert_int_equal(audio_monitor_speaker_layout(0x7FF, 7), SPEAKERS_UNKNOWN);
}

/* Counts past 8 give none, whatever the mask; nothing is truncated. */
static void large_counts_have_no_layout(void **state)
{
	(void)state;
	static const uint32_t counts[] = {9, 10, 16, 258, 0x80000002U, UINT32_MAX - 1, UINT32_MAX};

	for (size_t i = 0; i < ROW_COUNT; i++)
		for (size_t c = 0; c < sizeof(counts) / sizeof(counts[0]); c++)
			assert_int_equal(audio_monitor_speaker_layout(rows[i].mask, counts[c]), SPEAKERS_UNKNOWN);
}

struct tracker {
	uint64_t prev_video_ts;
	uint64_t time_since_prev;
};

static void track(struct tracker *t, uint64_t video_ts, uint64_t packet_ns)
{
	audio_monitor_track_video_ts(&t->prev_video_ts, &t->time_since_prev, video_ts, packet_ns);
}

/* The first packet stores the video timestamp and leaves the count. */
static void first_packet_stores_the_timestamp(void **state)
{
	(void)state;
	struct tracker t = {0, 0};
	track(&t, 1000 * MS, PACKET_NS);
	assert_int_equal(t.prev_video_ts, 1000 * MS);
	assert_int_equal(t.time_since_prev, 0);

	/* a count already there is left as it is */
	t.prev_video_ts = 0;
	t.time_since_prev = 42;
	track(&t, 7, PACKET_NS);
	assert_int_equal(t.prev_video_ts, 7);
	assert_int_equal(t.time_since_prev, 42);
}

/* A video timestamp of 0 is stored as 0, so every packet is the first one
 * again: the count never moves, the video never counts as still.
 * Characterized, not endorsed. */
static void zero_timestamp_is_always_first(void **state)
{
	(void)state;
	struct tracker t = {0, 0};
	for (int i = 0; i < 100; i++) {
		track(&t, 0, PACKET_NS);
		assert_int_equal(t.prev_video_ts, 0);
		assert_int_equal(t.time_since_prev, 0);
	}

	/* a count already there stays */
	t.time_since_prev = 500 * MS;
	track(&t, 0, PACKET_NS);
	assert_int_equal(t.time_since_prev, 500 * MS);

	/* video timestamp 0 after a real one: new, so the count resets and
	 * 0 is stored, and the next packet is the first again */
	t.prev_video_ts = 9000 * MS;
	t.time_since_prev = 300 * MS;
	track(&t, 0, PACKET_NS);
	assert_int_equal(t.prev_video_ts, 0);
	assert_int_equal(t.time_since_prev, 0);
	track(&t, 9000 * MS, PACKET_NS);
	assert_int_equal(t.prev_video_ts, 9000 * MS);
	assert_int_equal(t.time_since_prev, 0);
}

/* The same timestamp adds the packet's duration, 0 included. */
static void same_timestamp_counts(void **state)
{
	(void)state;
	struct tracker t = {0, 0};
	track(&t, 1000 * MS, PACKET_NS);
	track(&t, 1000 * MS, PACKET_NS);
	assert_int_equal(t.time_since_prev, PACKET_NS);
	track(&t, 1000 * MS, 0);
	assert_int_equal(t.time_since_prev, PACKET_NS);
	track(&t, 1000 * MS, 3);
	assert_int_equal(t.time_since_prev, PACKET_NS + 3);
	assert_int_equal(t.prev_video_ts, 1000 * MS);
}

/* The count wraps at 2^64. Characterized, not endorsed. */
static void count_wraps(void **state)
{
	(void)state;
	struct tracker t = {5, UINT64_MAX - 1};
	track(&t, 5, 3);
	assert_int_equal(t.time_since_prev, 1);
	assert_int_equal(t.prev_video_ts, 5);
}

/* Any other timestamp is new, earlier ones too: stored, count reset. */
static void timestamps_going_back_reset(void **state)
{
	(void)state;
	struct tracker t = {0, 0};
	track(&t, 5000 * MS, PACKET_NS);
	for (int i = 0; i < 10; i++)
		track(&t, 5000 * MS, PACKET_NS);
	assert_int_equal(t.time_since_prev, 10 * PACKET_NS);

	track(&t, 4000 * MS, PACKET_NS);
	assert_int_equal(t.prev_video_ts, 4000 * MS);
	assert_int_equal(t.time_since_prev, 0);

	track(&t, 4000 * MS - 1, PACKET_NS);
	assert_int_equal(t.prev_video_ts, 4000 * MS - 1);
	assert_int_equal(t.time_since_prev, 0);

	track(&t, UINT64_MAX, PACKET_NS);
	assert_int_equal(t.prev_video_ts, UINT64_MAX);
	assert_int_equal(t.time_since_prev, 0);
	track(&t, UINT64_MAX, PACKET_NS);
	assert_int_equal(t.time_since_prev, PACKET_NS);

	track(&t, 1, PACKET_NS);
	assert_int_equal(t.prev_video_ts, 1);
	assert_int_equal(t.time_since_prev, 0);
}

/* Stalled is past AUDIO_MONITOR_VIDEO_STALL_NS: still video counts up to
 * it packet by packet. */
static void still_video_reaches_the_stall(void **state)
{
	(void)state;
	struct tracker t = {0, 0};
	int packets = 0;
	track(&t, 2000 * MS, PACKET_NS);
	while (t.time_since_prev <= AUDIO_MONITOR_VIDEO_STALL_NS) {
		track(&t, 2000 * MS, PACKET_NS);
		packets++;
	}
	/* 21,333,333 ns a packet: 5 packets after the first */
	assert_int_equal(packets, 5);
	assert_int_equal(t.time_since_prev, 5 * PACKET_NS);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(every_mask_and_count),
		cmocka_unit_test(seven_channels_have_no_layout),
		cmocka_unit_test(large_counts_have_no_layout),
		cmocka_unit_test(first_packet_stores_the_timestamp),
		cmocka_unit_test(zero_timestamp_is_always_first),
		cmocka_unit_test(same_timestamp_counts),
		cmocka_unit_test(count_wraps),
		cmocka_unit_test(timestamps_going_back_reset),
		cmocka_unit_test(still_video_reaches_the_stall),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
