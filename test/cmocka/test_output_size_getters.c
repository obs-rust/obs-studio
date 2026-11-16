/* Actual-library characterization; keep unchanged through the Rust swap. */
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
	struct video_output_info info = {.name = "output-size",
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
	uint32_t (*indexed[])(const obs_output_t *, size_t) = {obs_output_get_width2, obs_output_get_height2};
	uint32_t (*defaults[])(const obs_output_t *) = {obs_output_get_width, obs_output_get_height};
	const char *indexed_names[] = {"obs_output_get_width2", "obs_output_get_height2"};
	const char *default_names[] = {"obs_output_get_width", "obs_output_get_height"};
	const uint32_t sizes[] = {0, 1, 1920, UINT32_MAX};
	for (size_t axis = 0; axis < 2; axis++) {
		char expected[256];
		log_count = 0;
		assert_int_equal(indexed[axis](NULL, SIZE_MAX), 0);
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		snprintf(expected, sizeof(expected), "%s: Null 'output' parameter", indexed_names[axis]);
		assert_string_equal(log_message, expected);
		log_count = 0;
		assert_int_equal(defaults[axis](NULL), 0);
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		snprintf(expected, sizeof(expected), "%s: Null 'output' parameter", default_names[axis]);
		assert_string_equal(log_message, expected);
		for (unsigned encoded = 0; encoded < 2; encoded++) {
			struct obs_output output = {0};
			output.context.name = "size-%s";
			output.info.flags = OBS_OUTPUT_AUDIO | (encoded ? OBS_OUTPUT_ENCODED : 0);
			output.video = video;
			output.scaled_width = 123;
			output.scaled_height = 456;
			log_count = 0;
			assert_int_equal(indexed[axis](&output, SIZE_MAX), 0);
			assert_int_equal(indexed[axis](&output, 0), 0);
			assert_int_equal(log_count, 2);
			assert_int_equal(log_level, LOG_WARNING);
			snprintf(expected, sizeof(expected), "Output 'size-%%s': Tried to use %s on a non-video output",
				 indexed_names[axis]);
			assert_string_equal(log_message, expected);
			log_count = 0;
			assert_int_equal(defaults[axis](&output), 0);
			assert_int_equal(log_count, 1);
			snprintf(expected, sizeof(expected), "Output 'size-%%s': Tried to use %s on a non-video output",
				 default_names[axis]);
			assert_string_equal(log_message, expected);
			output.info.flags |= OBS_OUTPUT_VIDEO;
			log_count = 0;
			assert_int_equal(indexed[axis](&output, MAX_OUTPUT_VIDEO_ENCODERS), 0);
			assert_int_equal(indexed[axis](&output, SIZE_MAX), 0);
			assert_int_equal(log_count, 0);
			for (unsigned media = 0; media < 2; media++) {
				output.video = media ? video : NULL;
				for (size_t x = 0; x < 4; x++) {
					for (size_t y = 0; y < 4; y++) {
						output.scaled_width = sizes[x];
						output.scaled_height = sizes[y];
						for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++) {
							struct obs_encoder encoder = {0};
							encoder.info.type = OBS_ENCODER_VIDEO;
							encoder.media = video;
							encoder.scaled_width = 640 + (uint32_t)idx;
							encoder.scaled_height = 480 + (uint32_t)idx;
							for (unsigned present = 0; present < 4; present++) {
								encoder.media = present == 3 ? NULL : video;
								encoder.scaled_width =
									present == 2 ? 0 : 640 + (uint32_t)idx;
								encoder.scaled_height =
									present == 2 ? 0 : 480 + (uint32_t)idx;
								output.video_encoders[idx] = present ? &encoder : NULL;
								uint32_t scaled = axis ? sizes[y] : sizes[x];
								uint32_t fallback = media ? (axis ? 240 : 320) : 0;
								/* Characterized, not endorsed: encoded outputs with an empty slot
								 * return zero even with scaled dimensions and video available. */
								uint32_t encoder_size =
									present == 2 ? (axis ? 240 : 320)
										     : (axis ? encoder.scaled_height
											     : encoder.scaled_width);
								uint32_t want = encoded ? (present && present != 3
												   ? encoder_size
												   : 0)
											: (scaled ? scaled : fallback);
								struct obs_output before;
								memcpy(&before, &output, sizeof(before));
								assert_int_equal(indexed[axis](&output, idx), want);
								assert_int_equal(defaults[axis](&output),
										 encoded && idx != 0 ? 0 : want);
								assert_memory_equal(&output, &before, sizeof(output));
							}
							output.video_encoders[idx] = NULL;
						}
					}
				}
			}
			assert_int_equal(log_count, 0);
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
