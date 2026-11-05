#include "../../libobs/obs-video-stats.h"

#include <stdlib.h>

#define CHECK(condition)         \
	do {                     \
		if (!(condition)) \
			abort();      \
	} while (0)

static struct obs_video_stats stats;

static void *publish_clock(void *unused)
{
	(void)unused;
	for (uint64_t i = 1; i <= 100000; i++)
		obs_video_stats_set_frame_time(&stats, (i << 32) | i);
	return NULL;
}

static void *read_clock(void *unused)
{
	(void)unused;
	uint64_t previous = 0;
	for (uint32_t i = 0; i < 100000; i++) {
		uint64_t timestamp = obs_video_stats_frame_time(&stats);
		CHECK((timestamp >> 32) == (uint32_t)timestamp);
		CHECK(timestamp >= previous);
		previous = timestamp;
	}
	return NULL;
}

int main(void)
{
	CHECK(obs_video_stats_init(&stats) == 0);
	CHECK(obs_video_stats_frame_time(&stats) == 0);
	pthread_t writer;
	pthread_t reader;
	CHECK(pthread_create(&writer, NULL, publish_clock, NULL) == 0);
	CHECK(pthread_create(&reader, NULL, read_clock, NULL) == 0);
	CHECK(pthread_join(writer, NULL) == 0);
	CHECK(pthread_join(reader, NULL) == 0);
	CHECK(obs_video_stats_frame_time(&stats) == ((UINT64_C(100000) << 32) | 100000));
	CHECK(obs_video_stats_total_frames(&stats) == 0);
	obs_video_stats_destroy(&stats);
}
