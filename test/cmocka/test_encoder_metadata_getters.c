/* Characterization of the actual libobs instance metadata getters. */
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
static char log_message[128];

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void expect_log(const char *function)
{
	char expected[128];
	snprintf(expected, sizeof(expected), "%s: Null 'encoder' parameter", function);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, expected);
	log_count = 0;
}

static void null_defaults(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	assert_null(obs_encoder_get_codec(NULL));
	expect_log("obs_encoder_get_codec");
	assert_int_equal(obs_encoder_get_type(NULL), OBS_ENCODER_AUDIO);
	expect_log("obs_encoder_get_type");
	assert_null(obs_encoder_get_id(NULL));
	expect_log("obs_encoder_get_id");
	assert_null(obs_encoder_get_type_data(NULL));
	expect_log("obs_encoder_get_type_data");
	assert_int_equal(obs_encoder_get_caps(NULL), 0);
	expect_log("obs_encoder_get_caps");
	base_set_log_handler(old_handler, old_param);
}

static void exact_independent_metadata(void **state)
{
	(void)state;
	char codec[] = "codec", id[] = "original-id", other[] = "rerouted", empty[] = "";
	int data = 17, original_data = 29;
	const char *strings[] = {NULL, empty, codec};
	const uint32_t caps[] = {0, 1, UINT32_C(0x80000000), UINT32_MAX};
	for (size_t i = 0; i < sizeof(strings) / sizeof(strings[0]); i++) {
		for (size_t j = 0; j < sizeof(caps) / sizeof(caps[0]); j++) {
			for (int type = OBS_ENCODER_AUDIO; type <= OBS_ENCODER_VIDEO; type++) {
				struct obs_encoder encoder = {0};
				encoder.info.codec = strings[i];
				encoder.info.type = (enum obs_encoder_type)type;
				encoder.info.id = other;
				encoder.info.type_data = &data;
				encoder.info.caps = ~caps[j];
				encoder.orig_info.codec = other;
				encoder.orig_info.type = type == OBS_ENCODER_AUDIO ? OBS_ENCODER_VIDEO
										   : OBS_ENCODER_AUDIO;
				encoder.orig_info.id = i == 0 ? NULL : id;
				encoder.orig_info.type_data = i == 0 ? NULL : &original_data;
				encoder.orig_info.caps = caps[j];
				struct obs_encoder before;
				memcpy(&before, &encoder, sizeof(before));
				assert_ptr_equal(obs_encoder_get_codec(&encoder), strings[i]);
				assert_int_equal(obs_encoder_get_type(&encoder), type);
				assert_ptr_equal(obs_encoder_get_id(&encoder), encoder.orig_info.id);
				assert_ptr_equal(obs_encoder_get_type_data(&encoder), encoder.orig_info.type_data);
				assert_int_equal(obs_encoder_get_caps(&encoder), caps[j]);
				assert_memory_equal(&encoder, &before, sizeof(encoder));
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults),
					   cmocka_unit_test(exact_independent_metadata)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
