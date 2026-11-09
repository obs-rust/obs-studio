/* Characterize the real libobs setter before extracting it for a Rust port.
 * Only reconnect_callback is read or written, so a zeroed private output is
 * sufficient: no OBS startup, worker threads, or partially owned resources. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>
#include <obs-internal.h>

static bool first_callback(void *data, obs_output_t *output, int code)
{
	return data == output && code == OBS_OUTPUT_DISCONNECTED;
}

static bool second_callback(void *data, obs_output_t *output, int code)
{
	return data == NULL && output != NULL && code == OBS_OUTPUT_SUCCESS;
}

static void installs_and_replaces_both_fields(void **state)
{
	(void)state;
	struct obs_output output = {0};
	int data = 7;

	obs_output_set_reconnect_callback(&output, first_callback, &output);
	assert_true(output.reconnect_callback.reconnect_cb == first_callback);
	assert_ptr_equal(output.reconnect_callback.param, &output);
	assert_true(output.reconnect_callback.reconnect_cb(output.reconnect_callback.param, &output,
							   OBS_OUTPUT_DISCONNECTED));

	obs_output_set_reconnect_callback(&output, second_callback, &data);
	assert_true(output.reconnect_callback.reconnect_cb == second_callback);
	assert_ptr_equal(output.reconnect_callback.param, &data);
	/* Installing a callback with NULL data does not clear the callback. */
	obs_output_set_reconnect_callback(&output, second_callback, NULL);
	assert_true(output.reconnect_callback.reconnect_cb == second_callback);
	assert_null(output.reconnect_callback.param);
	assert_true(
		output.reconnect_callback.reconnect_cb(output.reconnect_callback.param, &output, OBS_OUTPUT_SUCCESS));
}

static void null_callback_discards_the_supplied_data(void **state)
{
	(void)state;
	struct obs_output output = {0};
	int data = 7;

	obs_output_set_reconnect_callback(&output, first_callback, &output);
	/* Characterized, not endorsed: a NULL callback ignores non-NULL data. */
	obs_output_set_reconnect_callback(&output, NULL, &data);
	assert_null(output.reconnect_callback.reconnect_cb);
	assert_null(output.reconnect_callback.param);

	obs_output_set_reconnect_callback(&output, NULL, NULL);
	assert_null(output.reconnect_callback.reconnect_cb);
	assert_null(output.reconnect_callback.param);

	obs_output_set_reconnect_callback(&output, first_callback, &data);
	assert_true(output.reconnect_callback.reconnect_cb == first_callback);
	assert_ptr_equal(output.reconnect_callback.param, &data);
}

static void clearing_an_empty_callback_keeps_both_fields_empty(void **state)
{
	(void)state;
	struct obs_output output = {0};
	int data = 7;

	obs_output_set_reconnect_callback(&output, NULL, &data);
	assert_null(output.reconnect_callback.reconnect_cb);
	assert_null(output.reconnect_callback.param);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(installs_and_replaces_both_fields),
		cmocka_unit_test(null_callback_discards_the_supplied_data),
		cmocka_unit_test(clearing_an_empty_callback_keeps_both_fields_empty),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
