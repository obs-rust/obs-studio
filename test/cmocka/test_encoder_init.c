/* Characterize init_encoder through public creation, against the selected libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned callback_order;
static unsigned expected_callbacks;
static unsigned create_calls;
static int type_cookie;

static const char *encoder_name(void *data)
{
	assert_ptr_equal(data, &type_cookie);
	return "Initialization characterization";
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	create_calls++;
	return NULL;
}

static void encoder_destroy(void *data)
{
	(void)data;
	/* Creation alone must not instantiate the codec. */
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

static void defaults(obs_data_t *settings)
{
	assert_non_null(settings);
	assert_int_equal(callback_order, 0);
	callback_order = 1;
	obs_data_set_default_int(settings, "legacy", 11);
	obs_data_set_default_int(settings, "shared", 21);
}

static void defaults2(obs_data_t *settings, void *data)
{
	assert_ptr_equal(data, &type_cookie);
	assert_non_null(settings);
	assert_int_equal(callback_order, expected_callbacks & 1);
	if (expected_callbacks & 1) {
		assert_int_equal(obs_data_get_default_int(settings, "legacy"), 11);
		assert_int_equal(obs_data_get_default_int(settings, "shared"), 21);
	}
	callback_order = callback_order * 10 + 2;
	obs_data_set_default_int(settings, "modern", 12);
	obs_data_set_default_int(settings, "shared", 22);
}

static void check_mutex(pthread_mutex_t *mutex, bool recursive)
{
	assert_int_equal(pthread_mutex_trylock(mutex), 0);
	if (recursive) {
		assert_int_equal(pthread_mutex_trylock(mutex), 0);
		assert_int_equal(pthread_mutex_unlock(mutex), 0);
	}
	assert_int_equal(pthread_mutex_unlock(mutex), 0);
}

static void characterize(unsigned callbacks)
{
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_encoder_info info = {
		.id = "test_encoder_init",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = encoder_name,
		.create = encoder_create,
		.destroy = encoder_destroy,
		.encode = encoder_encode,
		.get_defaults = callbacks & 1 ? defaults : NULL,
		.get_defaults2 = callbacks & 2 ? defaults2 : NULL,
		.type_data = &type_cookie,
	};
	obs_register_encoder(&info);
	expected_callbacks = callbacks;
	create_calls = 0;
	for (unsigned supplied = 0; supplied < 2; supplied++) {
		obs_data_t *settings = supplied ? obs_data_create() : NULL;
		obs_data_t *hotkeys = supplied ? obs_data_create() : NULL;
		if (supplied) {
			obs_data_set_int(settings, "shared", 99);
			obs_data_set_string(hotkeys, "marker", "saved-hotkeys");
		}
		callback_order = 0;
		char name[] = "encoder-init-context";
		obs_encoder_t *encoder = obs_video_encoder_create(info.id, name, settings, hotkeys);
		assert_non_null(encoder);
		assert_int_equal(callback_order, callbacks == 3 ? 12 : callbacks);
		assert_int_equal(create_calls, 0);
		name[0] = 'X';
		assert_string_equal(obs_encoder_get_name(encoder), "encoder-init-context");
		assert_int_equal(encoder->context.type, OBS_OBJ_TYPE_ENCODER);
		assert_false(encoder->context.private);
		assert_null(encoder->context.uuid);
		assert_non_null(encoder->context.signals);
		assert_non_null(encoder->context.procs);
		assert_non_null(encoder->context.hotkey_data);
		assert_non_null(encoder->context.control);
		assert_null(encoder->context.data);
		obs_encoder_t *found = obs_get_encoder_by_name("encoder-init-context");
		assert_ptr_equal(found, encoder);
		obs_encoder_release(found);

		/* Context owns its settings/hotkeys after the caller releases its refs. */
		obs_data_release(settings);
		obs_data_release(hotkeys);
		settings = obs_encoder_get_settings(encoder);
		assert_non_null(settings);
		assert_ptr_equal(settings, encoder->context.settings);
		assert_int_equal(obs_data_get_default_int(settings, "legacy"), callbacks & 1 ? 11 : 0);
		assert_int_equal(obs_data_get_default_int(settings, "modern"), callbacks & 2 ? 12 : 0);
		int shared_default = callbacks & 2 ? 22 : (callbacks & 1 ? 21 : 0);
		assert_int_equal(obs_data_get_default_int(settings, "shared"), shared_default);
		assert_int_equal(obs_data_get_int(settings, "shared"), supplied ? 99 : shared_default);
		assert_string_equal(obs_data_get_string(encoder->context.hotkey_data, "marker"),
				    supplied ? "saved-hotkeys" : "");
		obs_data_release(settings);

		check_mutex(&encoder->init_mutex, true);
		check_mutex(&encoder->callbacks_mutex, true);
		check_mutex(&encoder->outputs_mutex, false);
		check_mutex(&encoder->pause.mutex, false);
		check_mutex(&encoder->roi_mutex, false);
		check_mutex(&encoder->last_error_mutex, false);
		obs_encoder_release(encoder);
		assert_null(obs_get_encoder_by_name("encoder-init-context"));
	}
	obs_shutdown();
}

static void no_defaults(void **state)
{
	(void)state;
	characterize(0);
}

static void legacy_defaults(void **state)
{
	(void)state;
	characterize(1);
}

static void modern_defaults(void **state)
{
	(void)state;
	characterize(2);
}

static void both_defaults(void **state)
{
	(void)state;
	characterize(3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(no_defaults), cmocka_unit_test(legacy_defaults),
					   cmocka_unit_test(modern_defaults), cmocka_unit_test(both_defaults)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
