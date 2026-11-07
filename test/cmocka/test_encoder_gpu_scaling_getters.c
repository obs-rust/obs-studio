/* Actual-library characterization of the public GPU scaling getter pair. */
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
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	assert_false(obs_encoder_gpu_scaling_enabled(NULL));
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_gpu_scaling_enabled: Null 'encoder' parameter");
	log_count = 0;
	assert_int_equal(obs_encoder_get_scale_type(NULL), OBS_SCALE_DISABLE);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_get_scale_type: Null 'encoder' parameter");
	for (int scale = OBS_SCALE_DISABLE; scale <= OBS_SCALE_AREA; scale++) {
		for (int type = OBS_ENCODER_AUDIO; type <= OBS_ENCODER_VIDEO; type++) {
			struct obs_encoder encoder = {0};
			encoder.context.name = "gpu-scaling-%s";
			encoder.info.type = (enum obs_encoder_type)type;
			encoder.orig_info.type = type == OBS_ENCODER_AUDIO ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
			encoder.gpu_scale_type = (enum obs_scale_type)scale;
			/* CPU scaled axes do not control either GPU getter. */
			encoder.scaled_width = scale == OBS_SCALE_DISABLE ? UINT32_MAX : 0;
			encoder.scaled_height = ~encoder.scaled_width;
			struct obs_encoder before;
			memcpy(&before, &encoder, sizeof(before));
			log_count = 0;
			assert_int_equal(obs_encoder_gpu_scaling_enabled(&encoder),
					 type == OBS_ENCODER_VIDEO && scale != OBS_SCALE_DISABLE);
			assert_int_equal(log_count, type == OBS_ENCODER_VIDEO ? 0 : 1);
			if (type == OBS_ENCODER_AUDIO) {
				assert_int_equal(log_level, LOG_WARNING);
				assert_string_equal(log_message,
						    "obs_encoder_gpu_scaling_enabled: encoder 'gpu-scaling-%s' is not a video encoder");
			}
			log_count = 0;
			assert_int_equal(obs_encoder_get_scale_type(&encoder), type == OBS_ENCODER_VIDEO ? scale : 0);
			assert_int_equal(log_count, type == OBS_ENCODER_VIDEO ? 0 : 1);
			if (type == OBS_ENCODER_AUDIO) {
				assert_int_equal(log_level, LOG_WARNING);
				assert_string_equal(log_message,
						    "obs_encoder_get_scale_type: encoder 'gpu-scaling-%s' is not a video encoder");
			}
			assert_memory_equal(&encoder, &before, sizeof(encoder));
		}
	}
	base_set_log_handler(old_handler, old_param);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(characterize)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
