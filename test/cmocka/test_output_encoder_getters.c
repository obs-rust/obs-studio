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
static log_handler_t saved_handler;
static void *saved_param;

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

static int setup(void **state)
{
	(void)state;
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(saved_handler, saved_param);
	return 0;
}

static void null_output(void **state)
{
	(void)state;
	assert_null(obs_output_get_video_encoder(NULL));
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_output_get_video_encoder: Null 'output' parameter");

	const size_t indices[] = {0, MAX_OUTPUT_AUDIO_ENCODERS, SIZE_MAX};
	for (size_t i = 0; i < sizeof(indices) / sizeof(indices[0]); i++) {
		log_count = 0;
		assert_null(obs_output_get_audio_encoder(NULL, indices[i]));
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(log_message, "obs_output_get_audio_encoder: Null 'output' parameter");
	}
}

static const uint32_t output_flags[] = {0,
					OBS_OUTPUT_VIDEO,
					OBS_OUTPUT_AUDIO,
					OBS_OUTPUT_ENCODED,
					OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED,
					OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK_VIDEO |
						OBS_OUTPUT_MULTI_TRACK_AUDIO,
					UINT32_MAX};

static void video_slots(void **state)
{
	(void)state;
	struct obs_encoder encoders[MAX_OUTPUT_VIDEO_ENCODERS] = {0};
	struct obs_encoder encoder_before[MAX_OUTPUT_VIDEO_ENCODERS];
	memcpy(encoder_before, encoders, sizeof(encoders));

	for (size_t flags = 0; flags < sizeof(output_flags) / sizeof(output_flags[0]); flags++) {
		struct obs_output output = {0};
		output.info.flags = output_flags[flags];
		struct obs_output before;
		memcpy(&before, &output, sizeof(before));
		assert_null(obs_output_get_video_encoder(&output));
		assert_memory_equal(&output, &before, sizeof(output));

		/* Characterized, not endorsed: even non-video and non-multitrack
		 * outputs return the first occupied slot, including a nonzero slot. */
		for (size_t first = 0; first < MAX_OUTPUT_VIDEO_ENCODERS; first++) {
			output.video_encoders[first] = &encoders[first];
			for (size_t last = first; last < MAX_OUTPUT_VIDEO_ENCODERS; last++) {
				output.video_encoders[last] = &encoders[last];
				memcpy(&before, &output, sizeof(before));
				assert_ptr_equal(obs_output_get_video_encoder(&output), &encoders[first]);
				assert_ptr_equal(obs_output_get_video_encoder(&output),
						 obs_output_get_video_encoder2(&output, first));
				assert_memory_equal(&output, &before, sizeof(output));
				assert_memory_equal(encoders, encoder_before, sizeof(encoders));
			}
			/* Removing the lowest occupied slot advances the borrowed result. */
			for (size_t idx = first; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++) {
				assert_ptr_equal(obs_output_get_video_encoder(&output), &encoders[idx]);
				output.video_encoders[idx] = NULL;
			}
			assert_null(obs_output_get_video_encoder(&output));
		}
	}
	assert_int_equal(log_count, 0);
}

static void audio_slots(void **state)
{
	(void)state;
	struct obs_encoder encoders[MAX_OUTPUT_AUDIO_ENCODERS] = {0};
	struct obs_encoder encoder_before[MAX_OUTPUT_AUDIO_ENCODERS];
	memcpy(encoder_before, encoders, sizeof(encoders));
	const size_t invalid_indices[] = {MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_AUDIO_ENCODERS + 1, SIZE_MAX};

	for (size_t flags = 0; flags < sizeof(output_flags) / sizeof(output_flags[0]); flags++) {
		struct obs_output output = {0};
		output.info.flags = output_flags[flags];
		for (size_t idx = 0; idx < MAX_OUTPUT_AUDIO_ENCODERS; idx++) {
			assert_null(obs_output_get_audio_encoder(&output, idx));
			output.audio_encoders[idx] = &encoders[idx];
		}
		/* Characterized, not endorsed: lookup does not check output flags
		 * or require multitrack support for a nonzero audio index. */
		for (size_t empty = 0; empty < MAX_OUTPUT_AUDIO_ENCODERS; empty++) {
			output.audio_encoders[empty] = NULL;
			struct obs_output before;
			memcpy(&before, &output, sizeof(before));
			for (size_t idx = 0; idx < MAX_OUTPUT_AUDIO_ENCODERS; idx++)
				assert_ptr_equal(obs_output_get_audio_encoder(&output, idx),
						 idx == empty ? NULL : &encoders[idx]);
			for (size_t i = 0; i < sizeof(invalid_indices) / sizeof(invalid_indices[0]); i++)
				assert_null(obs_output_get_audio_encoder(&output, invalid_indices[i]));
			assert_memory_equal(&output, &before, sizeof(output));
			assert_memory_equal(encoders, encoder_before, sizeof(encoders));
			output.audio_encoders[empty] = &encoders[empty];
		}
	}
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(null_output, setup, teardown),
					   cmocka_unit_test_setup_teardown(video_slots, setup, teardown),
					   cmocka_unit_test_setup_teardown(audio_slots, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
