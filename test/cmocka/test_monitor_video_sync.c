#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "monitor-video-sync.h"

/* Audio monitoring goes silent on Windows after a Media Source playing a
 * song with embedded cover art restarts (obsproject/obs-studio#9428,
 * obsproject/obs-studio#11739; upstream fix obsproject/obs-studio#14091).
 *
 * The WASAPI monitor holds the audio back while it runs ahead of the
 * source's video. Cover art is one picture: the video timestamp stays where
 * the picture was, so the monitor stops syncing to the video once that
 * timestamp has stood still for more than 100 ms. A restart decodes the
 * picture again and gives it a new timestamp. The monitor reset its count
 * on the new timestamp but never stored it, so every later packet saw a
 * "new" timestamp, the count stayed at 0, and the audio, ever further
 * ahead of the picture, was held back for good.
 *
 * The decision is shared code (monitor-video-sync.h) so it runs here on
 * every platform; the WASAPI monitor itself only runs on Windows. */

#define MS 1000000ULL
/* a 1024-frame packet at 48 kHz */
#define PACKET_NS (1024ULL * 1000000000ULL / 48000ULL)

struct monitor {
	uint64_t prev_video_ts;
	uint64_t time_since_prev;
};

static void feed(struct monitor *m, uint64_t video_ts, uint64_t ns)
{
	for (uint64_t t = 0; t < ns; t += PACKET_NS)
		audio_monitor_track_video_ts(&m->prev_video_ts, &m->time_since_prev, video_ts, PACKET_NS);
}

static void expect_stalled(const struct monitor *m, const char *what)
{
	if (m->time_since_prev <= AUDIO_MONITOR_VIDEO_STALL_NS) {
		fprintf(stderr, "%s: video stood still but only %llu ms counted (stall after %llu ms)\n", what,
			(unsigned long long)(m->time_since_prev / MS),
			(unsigned long long)(AUDIO_MONITOR_VIDEO_STALL_NS / MS));
		fail();
	}
}

/* First play: the cover's timestamp stands still, so the audio is played
 * without syncing to video. Passes on master. */
static void cover_art_first_play_stalls(void **state)
{
	(void)state;
	struct monitor m = {0};
	feed(&m, 1000 * MS, 500 * MS);
	expect_stalled(&m, "first play");
}

/* The report: the source restarts (scene switch, visibility toggle, loop)
 * and the cover gets a new timestamp, which then stands still too. */
static void cover_art_restart_stalls_again(void **state)
{
	(void)state;
	struct monitor m = {0};
	feed(&m, 1000 * MS, 500 * MS);
	feed(&m, 9000 * MS, 500 * MS);
	expect_stalled(&m, "after a restart");
	if (m.prev_video_ts != 9000 * MS) {
		fprintf(stderr, "after a restart: the monitor kept the old video timestamp %llu ms\n",
			(unsigned long long)(m.prev_video_ts / MS));
		fail();
	}
}

/* Each restart, and a looped file, again. */
static void cover_art_every_restart_stalls(void **state)
{
	(void)state;
	struct monitor m = {0};
	for (uint64_t i = 1; i <= 4; i++) {
		feed(&m, i * 7000 * MS, 300 * MS);
		expect_stalled(&m, "after repeated restarts");
	}
}

/* Guard: real video, a new timestamp every 33 ms, never counts as still,
 * so the monitor keeps syncing to it. */
static void moving_video_never_stalls(void **state)
{
	(void)state;
	struct monitor m = {0};
	for (uint64_t frame = 1; frame <= 300; frame++) {
		feed(&m, frame * 33 * MS, 33 * MS);
		if (m.time_since_prev > AUDIO_MONITOR_VIDEO_STALL_NS) {
			fprintf(stderr, "moving video: counted as still at frame %llu\n", (unsigned long long)frame);
			fail();
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(cover_art_first_play_stalls),
		cmocka_unit_test(cover_art_restart_stalls_again),
		cmocka_unit_test(cover_art_every_restart_stalls),
		cmocka_unit_test(moving_video_never_stalls),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
