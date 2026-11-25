/* Characterize the real obs_encoder_update API without live media or codec threads. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <cmocka.h>
#include "obs-internal.h"

struct fixture {
	struct obs_encoder encoder;
	struct obs_encoder_group group;
	unsigned calls;
	bool reenter;
	long long observed_bitrate;
};

static bool update(void *data, obs_data_t *settings)
{
	struct fixture *f = data;
	assert_ptr_equal(settings, f->encoder.context.settings);
	f->calls++;
	f->observed_bitrate = obs_data_get_int(settings, "bitrate");
	if (f->reenter) {
		f->reenter = false;
		obs_data_t *nested = obs_data_create();
		obs_data_set_int(nested, "bitrate", 3000);
		obs_encoder_update(&f->encoder, nested);
		obs_data_release(nested);
	}
	/* obs_encoder_update ignores the codec callback's success result. */
	return false;
}

static int setup(void **state)
{
	struct fixture *f = calloc(1, sizeof(*f));
	assert_non_null(f);
	f->encoder.context.settings = obs_data_create();
	assert_non_null(f->encoder.context.settings);
	obs_data_set_int(f->encoder.context.settings, "bitrate", 1000);
	obs_data_set_string(f->encoder.context.settings, "retained", "original");
	f->encoder.context.data = f;
	f->encoder.info.update = update;
	assert_int_equal(pthread_mutex_init(&f->group.mutex, NULL), 0);
	*state = f;
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	/* Only settings and the group mutex were allocated; no media, registry or codec owns this fixture. */
	obs_data_release(f->encoder.context.settings);
	assert_int_equal(pthread_mutex_destroy(&f->group.mutex), 0);
	free(f);
	return 0;
}

static void apply(struct fixture *f, long long bitrate)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "bitrate", bitrate);
	obs_data_set_string(settings, "added", "new");
	obs_encoder_update(&f->encoder, settings);
	obs_data_release(settings);
	assert_int_equal(obs_data_get_int(f->encoder.context.settings, "bitrate"), bitrate);
	assert_string_equal(obs_data_get_string(f->encoder.context.settings, "retained"), "original");
	assert_string_equal(obs_data_get_string(f->encoder.context.settings, "added"), "new");
}

static void null_encoder(void **state)
{
	struct fixture *f = *state;
	obs_encoder_update(NULL, f->encoder.context.settings);
	obs_encoder_update(NULL, NULL);
	assert_int_equal(f->calls, 0);
	assert_int_equal(obs_data_get_int(f->encoder.context.settings, "bitrate"), 1000);
}

static void settings_only_without_codec_data(void **state)
{
	struct fixture *f = *state;
	f->encoder.context.data = NULL;
	f->encoder.active = true;
	f->encoder.encoder_group = &f->group;
	apply(f, 2000);
	assert_int_equal(f->calls, 0);
	assert_false(f->encoder.reconfigure_requested);
	assert_int_equal(f->group.reconfigure_request, 0);
	obs_encoder_update(&f->encoder, NULL);
	assert_int_equal(obs_data_get_int(f->encoder.context.settings, "bitrate"), 2000);
}

static void settings_only_without_update_callback(void **state)
{
	struct fixture *f = *state;
	f->encoder.info.update = NULL;
	f->encoder.active = true;
	f->encoder.encoder_group = &f->group;
	apply(f, 2000);
	assert_int_equal(f->calls, 0);
	assert_false(f->encoder.reconfigure_requested);
	assert_int_equal(f->group.reconfigure_request, 0);
}

static void inactive_callback_and_null_settings(void **state)
{
	struct fixture *f = *state;
	obs_data_t *original = f->encoder.context.settings;
	f->encoder.encoder_group = &f->group;
	f->encoder.reconfigure_requested = true;
	apply(f, 2000);
	assert_int_equal(f->calls, 1);
	assert_int_equal(f->observed_bitrate, 2000);
	assert_ptr_equal(f->encoder.context.settings, original);
	assert_int_equal(f->group.reconfigure_request, 0);
	/* Existing pending state is preserved even though the inactive update is immediate. */
	assert_true(f->encoder.reconfigure_requested);
	obs_encoder_update(&f->encoder, NULL);
	assert_int_equal(f->calls, 2);
	assert_int_equal(f->observed_bitrate, 2000);
}

static void active_ungrouped_defers_and_coalesces(void **state)
{
	struct fixture *f = *state;
	f->encoder.active = true;
	apply(f, 2000);
	assert_true(f->encoder.reconfigure_requested);
	apply(f, 2500);
	assert_true(f->encoder.reconfigure_requested);
	assert_true(f->encoder.active);
	assert_int_equal(f->calls, 0);
}

static void active_group_first_request(void **state)
{
	struct fixture *f = *state;
	f->encoder.active = true;
	f->encoder.encoder_group = &f->group;
	f->group.encoders.num = 2;
	f->group.next_pts = 123;
	f->group.encoders_updated_next_pts = 1;
	f->group.encoders_reconfigured = 1;
	f->group.reconfigure_again = true;
	apply(f, 2000);
	assert_int_equal(f->group.reconfigure_request, 1);
	assert_int_equal(f->group.encoders_updated_next_pts, 0);
	assert_int_equal(f->group.encoders_reconfigured, 0);
	assert_false(f->group.reconfigure_again);
	assert_int_equal(f->group.next_pts, 123);
	assert_int_equal(f->encoder.last_handled_reconfigure_request, 0);
	assert_false(f->encoder.reconfigure_requested);
	assert_int_equal(f->calls, 0);
	assert_int_equal(pthread_mutex_trylock(&f->group.mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&f->group.mutex), 0);
}

static void active_group_pending_then_completed(void **state)
{
	struct fixture *f = *state;
	f->encoder.active = true;
	f->encoder.encoder_group = &f->group;
	f->group.encoders.num = 2;
	f->group.reconfigure_request = 4;
	f->group.next_pts = 456;
	f->group.encoders_updated_next_pts = 2;
	f->group.encoders_reconfigured = 1;
	f->encoder.last_handled_reconfigure_request = 3;
	apply(f, 2000);
	assert_int_equal(f->group.reconfigure_request, 4);
	assert_int_equal(f->group.encoders_updated_next_pts, 2);
	assert_int_equal(f->group.encoders_reconfigured, 1);
	assert_false(f->group.reconfigure_again);
	f->encoder.last_handled_reconfigure_request = 4;
	apply(f, 2100);
	assert_int_equal(f->group.reconfigure_request, 4);
	assert_true(f->group.reconfigure_again);
	f->group.encoders_reconfigured = 2;
	apply(f, 2200);
	assert_int_equal(f->group.reconfigure_request, 5);
	assert_int_equal(f->group.encoders_updated_next_pts, 0);
	assert_int_equal(f->group.encoders_reconfigured, 0);
	assert_false(f->group.reconfigure_again);
	assert_int_equal(f->group.next_pts, 456);
	assert_int_equal(f->encoder.last_handled_reconfigure_request, 4);
	assert_false(f->encoder.reconfigure_requested);
	assert_int_equal(f->calls, 0);
}

static void inactive_callback_can_reenter(void **state)
{
	struct fixture *f = *state;
	f->reenter = true;
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "bitrate", 2000);
	obs_encoder_update(&f->encoder, settings);
	obs_data_release(settings);
	assert_int_equal(f->calls, 2);
	assert_int_equal(f->observed_bitrate, 3000);
	assert_int_equal(obs_data_get_int(f->encoder.context.settings, "bitrate"), 3000);
	assert_false(f->encoder.reconfigure_requested);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_encoder, setup, teardown),
		cmocka_unit_test_setup_teardown(settings_only_without_codec_data, setup, teardown),
		cmocka_unit_test_setup_teardown(settings_only_without_update_callback, setup, teardown),
		cmocka_unit_test_setup_teardown(inactive_callback_and_null_settings, setup, teardown),
		cmocka_unit_test_setup_teardown(active_ungrouped_defers_and_coalesces, setup, teardown),
		cmocka_unit_test_setup_teardown(active_group_first_request, setup, teardown),
		cmocka_unit_test_setup_teardown(active_group_pending_then_completed, setup, teardown),
		cmocka_unit_test_setup_teardown(inactive_callback_can_reenter, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
