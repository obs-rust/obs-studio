/* Characterize obs_source_frame_init before extraction and Rust replacement
 * (#5619). No OBS core, video device, or graphics context is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs.h"
#include "media-io/video-frame.h"

static unsigned log_count;
static int log_level;
static char log_message[128];

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void null_frame_logs_the_caller(void **state)
{
	(void)state;
	log_handler_t saved_handler;
	void *saved_param;
	base_get_log_handler(&saved_handler, &saved_param);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
	base_set_log_handler(capture_log, NULL);
	obs_source_frame_init(NULL, VIDEO_FORMAT_RGBA, 13, 7);
	base_set_log_handler(saved_handler, saved_param);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_frame_init: Null 'frame' parameter");
}

static void check_frame(enum video_format format, uint32_t width, uint32_t height)
{
	struct video_frame video;
	struct obs_source_frame frame, expected;
	/* Characterized, not endorsed: initialization does not reset timestamp,
	 * color metadata, flags, refs, or any other unrelated bytes. */
	memset(&frame, 0x5a, sizeof(frame));
	memcpy(&expected, &frame, sizeof(frame));
	video_frame_init(&video, format, width, height);
	obs_source_frame_init(&frame, format, width, height);

	expected.format = format;
	expected.width = width;
	expected.height = height;
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		assert_int_equal(frame.linesize[i], video.linesize[i]);
		if (video.data[i]) {
			assert_non_null(frame.data[i]);
			assert_int_equal((uintptr_t)frame.data[i] - (uintptr_t)frame.data[0],
					 (uintptr_t)video.data[i] - (uintptr_t)video.data[0]);
		} else {
			assert_null(frame.data[i]);
		}
		expected.data[i] = frame.data[i];
		expected.linesize[i] = video.linesize[i];
	}
	assert_memory_equal(&frame, &expected, sizeof(frame));
	video_frame_free(&video);
	obs_source_frame_free(&frame);
}

static void plane_layout_and_unrelated_bytes(void **state)
{
	(void)state;
	const enum video_format formats[] = {VIDEO_FORMAT_RGBA, VIDEO_FORMAT_I420, VIDEO_FORMAT_NV12,
					     VIDEO_FORMAT_I444, VIDEO_FORMAT_YUVA, VIDEO_FORMAT_P010};
	for (size_t i = 0; i < sizeof(formats) / sizeof(formats[0]); i++) {
		check_frame(formats[i], 1, 1);
		check_frame(formats[i], 13, 7);
		check_frame(formats[i], 32, 16);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_frame_logs_the_caller),
		cmocka_unit_test(plane_layout_and_unrelated_bytes),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
