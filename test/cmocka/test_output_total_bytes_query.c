/* Actual-library characterization for #5203. Keep unchanged through the Rust swap.
 * Stack-owned private storage needs neither OBS startup nor a device. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct query_log {
	unsigned calls;
	int level;
	char message[128];
};

struct query_data {
	unsigned calls;
	unsigned callback;
	uint64_t result;
};

static void *last_data;

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct query_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static uint64_t first_callback(void *data)
{
	last_data = data;
	if (!data)
		return UINT64_MAX;
	struct query_data *query = data;
	query->calls++;
	query->callback = 1;
	return query->result;
}

static uint64_t second_callback(void *data)
{
	last_data = data;
	struct query_data *query = data;
	query->calls++;
	query->callback = 2;
	return query->result;
}

static void null_output(void **state)
{
	(void)state;
	log_handler_t old;
	void *param;
	struct query_log log = {0};
	base_get_log_handler(&old, &param);
	base_set_log_handler(capture_log, &log);
	uint64_t result = obs_output_get_total_bytes(NULL);
	base_set_log_handler(old, param);
	assert_true(result == 0);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_output_get_total_bytes: Null 'output' parameter");
}

static void callback_delay_matrix(void **state)
{
	(void)state;
	struct obs_output output = {0};
	unsigned char before[sizeof(output)];
	struct query_data first = {0}, second = {0};
	uint64_t (*callbacks[])(void *) = {NULL, first_callback, second_callback};
	const uint64_t results[] = {0, 1, UINT64_C(0x100000000), UINT64_C(0xfedcba9876543210), UINT64_MAX};
	output.owns_info_id = true; /* Unrelated private state must not change. */
	for (size_t cb = 0; cb < sizeof(callbacks) / sizeof(callbacks[0]); cb++) {
		for (unsigned flags = 0; flags < 4; flags++) {
			for (size_t i = 0; i < sizeof(results) / sizeof(results[0]); i++) {
				struct query_data *data = i % 2 ? &first : &second;
				first.calls = second.calls = 0;
				first.callback = second.callback = 0;
				data->result = results[i];
				last_data = NULL;
				output.context.data = data;
				output.info.get_total_bytes = callbacks[cb];
				output.delay_active = (flags & 1) != 0;
				output.delay_capturing = (flags & 2) != 0;
				memcpy(before, &output, sizeof(output));
				bool dispatch = cb != 0 && flags != 1;
				uint64_t result = obs_output_get_total_bytes(&output);
				assert_true(result == (dispatch ? results[i] : 0));
				assert_memory_equal(&output, before, sizeof(output));
				assert_int_equal(first.calls + second.calls, dispatch ? 1 : 0);
				assert_int_equal(data->callback, dispatch ? cb : 0);
				assert_ptr_equal(last_data, dispatch ? data : NULL);
			}
		}
	}
	/* Characterized, not endorsed: a callback receives a NULL backend token
	 * verbatim. The wrapper must not add its own data-pointer guard. */
	output.info.get_total_bytes = first_callback;
	output.context.data = NULL;
	output.delay_active = false;
	last_data = &first;
	assert_true(obs_output_get_total_bytes(&output) == UINT64_MAX);
	assert_null(last_data);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_output), cmocka_unit_test(callback_delay_matrix)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
