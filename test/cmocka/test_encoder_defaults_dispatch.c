/* Pin encoder defaults before the separate Rust swap (Refs #5093). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-encoder-defaults.c.h")
#error "encoder defaults extraction seam is missing"
#endif

static log_handler_t saved_handler;
static void *saved_param;
static unsigned log_count;
static int log_level;
static char log_message[256];
static unsigned calls;
static bool expect_legacy;
static void *expected_type_data;
static obs_data_t *callback_settings;
static int type_token;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void legacy_defaults(obs_data_t *settings)
{
	assert_int_equal(calls++, 0);
	assert_non_null(settings);
	callback_settings = settings;
	obs_data_set_default_int(settings, "winner", 11);
	obs_data_set_default_bool(settings, "legacy", true);
}

static void modern_defaults(obs_data_t *settings, void *type_data)
{
	assert_int_equal(calls++, expect_legacy ? 1 : 0);
	assert_ptr_equal(type_data, expected_type_data);
	if (expect_legacy) {
		assert_ptr_equal(settings, callback_settings);
		assert_int_equal(obs_data_get_int(settings, "winner"), 11);
		assert_true(obs_data_get_bool(settings, "legacy"));
	} else {
		assert_false(obs_data_has_default_value(settings, "winner"));
	}
	callback_settings = settings;
	obs_data_set_default_int(settings, "winner", 22);
}

static void wrong_defaults(obs_data_t *settings)
{
	(void)settings;
	fail();
}

static const char *encoder_name(void *type_data)
{
	(void)type_data;
	return "defaults characterization";
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	fail(); /* Defaults queries must not create an encoder. */
	return NULL;
}

static void encoder_destroy(void *data)
{
	(void)data;
	fail();
}

static bool encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received;
	fail();
	return false;
}

static const char *const ids[] = {"defaults-none", "defaults-legacy", "defaults-modern", "defaults-both"};

static struct obs_encoder_info info_for(unsigned mask)
{
	struct obs_encoder_info info = {
		.id = ids[mask],
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = encoder_name,
		.create = encoder_create,
		.destroy = encoder_destroy,
		.encode = encoder_encode,
		.get_defaults = (mask & 1) ? legacy_defaults : NULL,
		.get_defaults2 = (mask & 2) ? modern_defaults : NULL,
		.type_data = &type_token,
	};
	return info;
}

static int setup(void **state)
{
	(void)state;
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	assert_true(obs_startup("en-US", NULL, NULL));
	for (unsigned mask = 0; mask < 4; mask++) {
		struct obs_encoder_info info = info_for(mask);
		obs_register_encoder(&info);
	}
	log_count = 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	base_set_log_handler(saved_handler, saved_param);
	return 0;
}

static obs_data_t *query(unsigned mask, obs_encoder_t *encoder)
{
	calls = 0;
	callback_settings = NULL;
	expect_legacy = (mask & 1) != 0;
	expected_type_data = encoder ? encoder->info.type_data : &type_token;
	obs_data_t *settings = encoder ? obs_encoder_get_defaults(encoder) : obs_encoder_defaults(ids[mask]);
	assert_non_null(settings);
	assert_int_equal(calls, ((mask & 1) != 0) + ((mask & 2) != 0));
	if (mask) {
		assert_ptr_equal(settings, callback_settings);
		assert_int_equal(obs_data_get_int(settings, "winner"), (mask & 2) ? 22 : 11);
		assert_true(obs_data_has_default_value(settings, "winner"));
		assert_false(obs_data_has_user_value(settings, "winner"));
	} else {
		assert_null(obs_data_first(settings));
	}
	assert_false(obs_data_has_user_value(settings, "caller-only"));
	assert_false(obs_data_has_user_value(settings, "existing"));
	return settings;
}

static void fresh_owned(unsigned mask, obs_encoder_t *encoder)
{
	obs_data_t *first = query(mask, encoder);
	obs_data_set_int(first, "caller-only", 99);
	obs_data_set_default_int(first, "winner", 77);
	obs_data_t *second = query(mask, encoder);
	assert_ptr_not_equal(first, second);
	if (encoder) {
		assert_ptr_not_equal(first, encoder->context.settings);
		assert_ptr_not_equal(second, encoder->context.settings);
		assert_int_equal(obs_data_get_int(encoder->context.settings, "existing"), 123);
		assert_false(obs_data_has_default_value(encoder->context.settings, "winner"));
	}
	obs_data_release(first);
	/* Releasing one result must not invalidate the independently owned other. */
	obs_data_set_int(second, "caller-only", 42);
	obs_data_addref(second);
	obs_data_release(second);
	assert_int_equal(obs_data_get_int(second, "caller-only"), 42);
	obs_data_release(second);
	obs_data_t *third = query(mask, encoder);
	obs_data_release(third);
}

static void by_id(void **state)
{
	(void)state;
	assert_null(obs_encoder_defaults("unregistered-defaults-id"));
	for (unsigned mask = 0; mask < 4; mask++)
		fresh_owned(mask, NULL);
	assert_int_equal(log_count, 0);
}

static void by_instance(void **state)
{
	(void)state;
	obs_encoder_t encoder = {0};
	encoder.orig_info.get_defaults = wrong_defaults;
	encoder.context.settings = obs_data_create();
	obs_data_set_int(encoder.context.settings, "existing", 123);
	for (unsigned mask = 0; mask < 4; mask++) {
		encoder.info = info_for(mask);
		/* Deliberately disagree with the registered ID: dispatch uses active info. */
		encoder.info.id = ids[3 - mask];
		int instance_token = 0;
		encoder.info.type_data = &instance_token;
		fresh_owned(mask, &encoder);
		encoder.info.type_data = NULL;
		fresh_owned(mask, &encoder);
	}
	obs_data_release(encoder.context.settings);
	assert_int_equal(log_count, 0);
}

static void null_instance(void **state)
{
	(void)state;
	assert_null(obs_encoder_get_defaults(NULL));
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	/* Characterized, not endorsed: diagnostic names the by-ID API. */
	assert_string_equal(log_message, "obs_encoder_defaults: Null 'encoder' parameter");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(by_id, setup, teardown),
		cmocka_unit_test_setup_teardown(by_instance, setup, teardown),
		cmocka_unit_test_setup_teardown(null_instance, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
