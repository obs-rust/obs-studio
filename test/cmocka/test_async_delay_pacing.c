/* A skipped graphics tick must not change the Video Delay filter's delay
 * (#3368): changing the displayed frame's time moves audio placement too. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include "plugins/obs-filters/build/obs-filters.pch.h"

/* These frames are owned by this test, not a source's async frame cache. */
static obs_source_t *test_delay_parent(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return NULL;
}

const char *obs_module_text(const char *text)
{
	return text;
}

#define obs_filter_get_parent test_delay_parent
#include "plugins/obs-filters/async-delay-filter.c.h"
#undef obs_filter_get_parent

static void feed(struct async_delay_data *filter, uint64_t milliseconds, int64_t expected)
{
	struct obs_source_frame *frame = obs_source_frame_create(VIDEO_FORMAT_BGRA, 1, 1);
	assert_non_null(frame);
	frame->timestamp = milliseconds * 1000000;
	struct obs_source_frame *output = async_delay_filter_video(filter, frame);
	if (expected < 0) {
		assert_null(output);
	} else {
		assert_non_null(output);
		uint64_t timestamp = output->timestamp;
		obs_source_release_frame(NULL, output);
		assert_int_equal(timestamp, (uint64_t)expected * 1000000);
	}
}

static void skipped_tick_drops_expired_frames(void **state)
{
	UNUSED_PARAMETER(state);
	struct async_delay_data filter = {.interval = 3000000000};
	for (uint64_t ms = 0; ms < 3000; ms += 100)
		feed(&filter, ms, -1);
	feed(&filter, 3000, 0);
	feed(&filter, 3100, 100);
	/* The graphics thread skipped two ticks. Do not replay their old
	 * queued frames now and increase the delay by 200 ms. */
	feed(&filter, 3400, 400);
	feed(&filter, 3500, 500);
	free_video_data(&filter, NULL);
	deque_free(&filter.video_frames);
}

static void timestamp_gap_holds_until_frame_is_due(void **state)
{
	UNUSED_PARAMETER(state);
	struct async_delay_data filter = {.interval = 3000000000};
	for (uint64_t ms = 0; ms < 3000; ms += 100) {
		if (ms > 1000 && ms < 1400)
			continue;
		feed(&filter, ms, -1);
	}
	for (uint64_t ms = 3000; ms <= 4000; ms += 100)
		feed(&filter, ms, (int64_t)ms - 3000);
	/* There was no frame at 1100, 1200 or 1300. Hold the last picture
	 * instead of presenting frame 1400 early and moving audio backwards. */
	feed(&filter, 4100, -1);
	feed(&filter, 4200, -1);
	feed(&filter, 4300, -1);
	feed(&filter, 4400, 1400);
	free_video_data(&filter, NULL);
	deque_free(&filter.video_frames);
}

static void zero_delay_returns_current_frame(void **state)
{
	UNUSED_PARAMETER(state);
	struct async_delay_data filter = {0};
	feed(&filter, 0, 0);
	feed(&filter, 100, 100);
	feed(&filter, 700, 700);
	assert_int_equal(filter.video_frames.size, 0);
	deque_free(&filter.video_frames);
}

static void timestamp_reset_refills_delay(void **state)
{
	UNUSED_PARAMETER(state);
	struct async_delay_data filter = {.interval = 3000000000};
	for (uint64_t ms = 0; ms < 3000; ms += 100)
		feed(&filter, ms, -1);
	feed(&filter, 3000, 0);
	/* Rewinding the source must discard the old timeline. */
	for (uint64_t ms = 100; ms < 3100; ms += 100)
		feed(&filter, ms, -1);
	feed(&filter, 3100, 100);
	/* A forward timestamp jump over one second refills as well. */
	for (uint64_t ms = 5000; ms < 8000; ms += 100)
		feed(&filter, ms, -1);
	feed(&filter, 8000, 5000);
	free_video_data(&filter, NULL);
	deque_free(&filter.video_frames);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(skipped_tick_drops_expired_frames),
		cmocka_unit_test(timestamp_gap_holds_until_frame_is_due),
		cmocka_unit_test(zero_delay_returns_current_frame),
		cmocka_unit_test(timestamp_reset_refills_delay),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
