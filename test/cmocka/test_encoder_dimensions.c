/* Actual-library characterization of public width and height getters. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[256];
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

static void characterize(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	video_t *video = NULL;
	struct video_output_info info = {.name = "dimensions",
					 .format = VIDEO_FORMAT_RGBA,
					 .fps_num = 30,
					 .fps_den = 1,
					 .width = 320,
					 .height = 240,
					 .cache_size = 1};
	assert_int_equal(video_output_open(&video, &info), VIDEO_OUTPUT_SUCCESS);
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	uint32_t (*getters[])(const obs_encoder_t *) = {obs_encoder_get_width, obs_encoder_get_height};
	const char *names[] = {"obs_encoder_get_width", "obs_encoder_get_height"};
	const uint32_t sizes[] = {0, 1, 1920, UINT32_C(0x80000000), UINT32_MAX};
	for (size_t axis = 0; axis < 2; axis++) {
		log_count = 0;
		assert_int_equal(getters[axis](NULL), 0);
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		char expected[256];
		snprintf(expected, sizeof(expected), "%s: Null 'encoder' parameter", names[axis]);
		assert_string_equal(log_message, expected);
		for (int type = OBS_ENCODER_AUDIO; type <= OBS_ENCODER_VIDEO; type++) {
			for (int media = 0; media < 2; media++) {
				for (size_t x = 0; x < 5; x++) {
					for (size_t y = 0; y < 5; y++) {
						struct obs_encoder encoder = {0}, before;
						encoder.context.name = "dimensions-%s";
						encoder.info.type = (enum obs_encoder_type)type;
						encoder.orig_info.type = type == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO
												   : OBS_ENCODER_VIDEO;
						encoder.media = media ? video : NULL;
						encoder.scaled_width = sizes[x];
						encoder.scaled_height = sizes[y];
						memcpy(&before, &encoder, sizeof(before));
						uint32_t scaled = axis ? sizes[y] : sizes[x];
						uint32_t fallback = axis ? 240 : 320;
						log_count = 0;
						/* Characterized, not endorsed: no media hides a nonzero scaled size. */
						assert_int_equal(getters[axis](&encoder),
								 type == OBS_ENCODER_VIDEO && media
									 ? (scaled ? scaled : fallback)
									 : 0);
						assert_int_equal(log_count, type == OBS_ENCODER_VIDEO ? 0 : 1);
						if (type == OBS_ENCODER_AUDIO) {
							assert_int_equal(log_level, LOG_WARNING);
							snprintf(expected, sizeof(expected),
								 "%s: encoder 'dimensions-%%s' is not a video encoder",
								 names[axis]);
							assert_string_equal(log_message, expected);
						}
						assert_memory_equal(&encoder, &before, sizeof(encoder));
					}
				}
			}
		}
	}
	base_set_log_handler(old_handler, old_param);
	video_output_close(video);
	obs_shutdown();
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(characterize)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
