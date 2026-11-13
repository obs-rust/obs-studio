/* Characterize obs_sceneitem_get_private_settings before its Rust swap (Refs #4819).
 * Stack item/source fixtures isolate the getter; data handles are real library
 * objects and no private obs_data layout is assumed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "obs-scene.h"

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
	obs_data_t *null_result = obs_sceneitem_get_private_settings(NULL);
	struct obs_scene_item item = {0}, before;
	memcpy(&before, &item, sizeof(item));
	obs_data_t *missing_result = obs_sceneitem_get_private_settings(&item);
	base_set_log_handler(previous, param);

	assert_null(null_result);
	assert_null(missing_result);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_sceneitem_get_private_settings: Null 'item' parameter");
	/* Characterized, not endorsed: stored NULL is not replaced or diagnosed. */
	assert_memory_equal(&item, &before, sizeof(item));
}

static void retained_identity_and_source_independence(void **state)
{
	(void)state;
	const size_t counts[] = {1, 2, 5};
	for (size_t n = 0; n < sizeof(counts) / sizeof(counts[0]); n++) {
		struct obs_source source = {0}, source_before;
		struct obs_scene_item item = {.source = &source}, item_before;
		item.private_settings = obs_data_create();
		source.private_settings = obs_data_create();
		source.context.settings = obs_data_create();
		assert_non_null(item.private_settings);
		assert_non_null(source.private_settings);
		assert_non_null(source.context.settings);
		obs_data_set_int(item.private_settings, "value", 17);
		obs_data_set_int(source.private_settings, "value", 91);
		obs_data_set_int(source.context.settings, "value", 123);
		memcpy(&item_before, &item, sizeof(item));
		memcpy(&source_before, &source, sizeof(source));
		obs_data_t *source_settings = obs_source_get_private_settings(&source);
		assert_ptr_equal(source_settings, source.private_settings);
		obs_data_t *retained[5];
		for (size_t i = 0; i < counts[n]; i++) {
			retained[i] = obs_sceneitem_get_private_settings(&item);
			assert_ptr_equal(retained[i], item.private_settings);
			assert_ptr_not_equal(retained[i], source_settings);
			assert_int_equal(obs_data_get_int(retained[i], "value"), 17);
			assert_memory_equal(&item, &item_before, sizeof(item));
			assert_memory_equal(&source, &source_before, sizeof(source));
		}

		/* Calls share the item's stored object, independently of its source. */
		obs_data_set_int(retained[0], "value", 42);
		assert_int_equal(obs_data_get_int(item.private_settings, "value"), 42);
		assert_int_equal(obs_data_get_int(source_settings, "value"), 91);
		obs_data_set_int(source_settings, "value", 99);
		assert_int_equal(obs_data_get_int(retained[0], "value"), 42);
		assert_int_equal(obs_data_get_int(source.context.settings, "value"), 123);
		obs_data_item_t *witness = obs_data_item_byname(item.private_settings, "value");
		assert_non_null(witness);
		obs_data_release(item.private_settings);
		item.private_settings = NULL;
		for (size_t i = 0; i < counts[n]; i++) {
			assert_int_equal(obs_data_get_int(retained[i], "value"), 42);
			obs_data_release(retained[i]);
		}
		/* Exactly one retain per call lets the last release destroy the data
		 * object, leaving only this item reference. An excess retain leaves
		 * the data owning the item, so releasing the witness won't clear it. */
		obs_data_item_release(&witness);
		assert_null(witness);
		assert_int_equal(obs_data_get_int(source_settings, "value"), 99);
		obs_data_release(source.private_settings);
		source.private_settings = NULL;
		assert_int_equal(obs_data_get_int(source_settings, "value"), 99);
		obs_data_release(source_settings);
		assert_int_equal(obs_data_get_int(source.context.settings, "value"), 123);
		obs_data_release(source.context.settings);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_and_missing_settings),
		cmocka_unit_test(retained_identity_and_source_independence),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
