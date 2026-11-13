/* Characterize obs_source_get_private_settings before its Rust swap (Refs #4789).
 * A stack source isolates the getter from unrelated source lifecycle work. Data
 * handles are real library objects; no private obs_data layout is assumed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct log_capture {
	unsigned calls;
	int level;
	char message[160];
};

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct log_capture *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void null_and_missing_settings(void **state)
{
	(void)state;
	struct log_capture log = {0};
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	base_set_log_handler(capture_log, &log);
	obs_data_t *null_result = obs_source_get_private_settings(NULL);
	struct obs_source source = {0}, before;
	memcpy(&before, &source, sizeof(source));
	obs_data_t *missing_result = obs_source_get_private_settings(&source);
	base_set_log_handler(previous, param);

	assert_null(null_result);
	assert_null(missing_result);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_source_get_private_settings: Null 'source' parameter");
	/* Characterized, not endorsed: a stored NULL is not replaced or diagnosed. */
	assert_memory_equal(&source, &before, sizeof(source));
}

static void retained_identity(void **state)
{
	(void)state;
	const size_t counts[] = {1, 2, 5};
	for (size_t n = 0; n < sizeof(counts) / sizeof(counts[0]); n++) {
		struct obs_source source = {0}, before;
		source.private_settings = obs_data_create();
		source.context.settings = obs_data_create();
		assert_non_null(source.private_settings);
		assert_non_null(source.context.settings);
		obs_data_set_int(source.private_settings, "value", 17);
		obs_data_set_int(source.context.settings, "value", 91);
		memcpy(&before, &source, sizeof(source));
		obs_data_t *retained[5];
		for (size_t i = 0; i < counts[n]; i++) {
			retained[i] = obs_source_get_private_settings(&source);
			assert_ptr_equal(retained[i], source.private_settings);
			assert_int_equal(obs_data_get_int(retained[i], "value"), 17);
			assert_memory_equal(&source, &before, sizeof(source));
		}

		/* All returned handles share the stored object, not a copy. */
		obs_data_set_int(retained[0], "value", 42);
		assert_int_equal(obs_data_get_int(source.private_settings, "value"), 42);
		assert_int_equal(obs_data_get_int(source.context.settings, "value"), 91);
		obs_data_item_t *witness = obs_data_item_byname(source.private_settings, "value");
		assert_non_null(witness);
		obs_data_release(source.private_settings);
		source.private_settings = NULL;
		for (size_t i = 0; i < counts[n]; i++) {
			assert_int_equal(obs_data_get_int(retained[i], "value"), 42);
			obs_data_release(retained[i]);
		}
		/* The data object owns one item reference. With exactly one retain
		 * per call, the last data release leaves only our witness reference,
		 * and releasing that reference clears the pointer. An excess retain
		 * keeps the item owned and leaves witness non-NULL. */
		obs_data_item_release(&witness);
		assert_null(witness);
		assert_int_equal(obs_data_get_int(source.context.settings, "value"), 91);
		obs_data_release(source.context.settings);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_and_missing_settings),
		cmocka_unit_test(retained_identity),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
