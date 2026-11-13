/* Characterize the public getter through the actual hybrid and C-reference
 * libraries. A non-NULL encoder is a precondition; plugin data may be NULL. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned callback_count;
static void *callback_data;
static uint32_t callback_result;

static uint32_t priming_callback(void *data)
{
	callback_count++;
	callback_data = data;
	return callback_result;
}

static void absent_callback_returns_zero(void **state)
{
	(void)state;
	uint32_t token = 123;
	struct obs_encoder encoder = {0};
	/* orig_info is not the dispatch table. */
	encoder.orig_info.get_priming_samples = priming_callback;
	callback_count = 0;
	for (unsigned has_data = 0; has_data < 2; has_data++) {
		encoder.context.data = has_data ? &token : NULL;
		assert_int_equal(obs_encoder_get_priming_samples(&encoder), 0);
		assert_int_equal(callback_count, 0);
		assert_ptr_equal(encoder.context.data, has_data ? &token : NULL);
	}
	assert_int_equal(token, 123);
}

static void callback_receives_exact_data_once(void **state)
{
	(void)state;
	uint32_t token = 456;
	struct obs_encoder encoder = {0};
	encoder.info.get_priming_samples = priming_callback;
	const uint32_t results[] = {0, 1, 1024, UINT32_C(0x80000000), UINT32_MAX};
	for (unsigned has_data = 0; has_data < 2; has_data++) {
		encoder.context.data = has_data ? &token : NULL;
		for (size_t i = 0; i < sizeof(results) / sizeof(results[0]); i++) {
			callback_count = 0;
			/* Distinct from the expected pointer, including the NULL case. */
			callback_data = &encoder;
			callback_result = results[i];
			assert_int_equal(obs_encoder_get_priming_samples(&encoder), results[i]);
			assert_int_equal(callback_count, 1);
			assert_ptr_equal(callback_data, encoder.context.data);
			assert_ptr_equal(encoder.info.get_priming_samples, priming_callback);
			assert_ptr_equal(encoder.context.data, has_data ? &token : NULL);
		}
	}
	assert_int_equal(token, 456);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(absent_callback_returns_zero),
					   cmocka_unit_test(callback_receives_exact_data_once)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
