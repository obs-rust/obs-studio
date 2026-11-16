/* Actual-library characterization of obs_output_update, kept through the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct update_observation {
	void *data;
	obs_data_t *settings;
	unsigned calls;
	long long values[2];
	bool returned;
};

static struct update_observation observation;

static void observe_update(void *data, obs_data_t *settings)
{
	assert_false(observation.returned);
	assert_ptr_equal(data, observation.data);
	assert_ptr_equal(settings, observation.settings);
	assert_true(observation.calls < 2);
	observation.values[observation.calls++] = obs_data_get_int(settings, "overwrite");
	assert_string_equal(obs_data_get_string(settings, "preserved"), "original");
	assert_true(obs_data_get_bool(settings, "added"));
}

static void merge_before_callback(void **state)
{
	(void)state;
	int plugin_context;
	obs_data_t *stored = obs_data_create();
	obs_data_t *incoming = obs_data_create();
	obs_data_set_int(stored, "overwrite", 10);
	obs_data_set_string(stored, "preserved", "original");
	obs_data_set_int(incoming, "overwrite", 20);
	obs_data_set_bool(incoming, "added", true);
	struct obs_output output = {0};
	output.context.settings = stored;
	output.context.data = &plugin_context;
	output.info.update = observe_update;
	observation = (struct update_observation){.data = &plugin_context, .settings = stored};

	obs_output_update(&output, incoming);
	observation.returned = true;
	assert_int_equal(observation.calls, 1);
	assert_int_equal(observation.values[0], 20);
	assert_ptr_equal(output.context.settings, stored);
	assert_int_equal(obs_data_get_int(stored, "overwrite"), 20);
	assert_string_equal(obs_data_get_string(stored, "preserved"), "original");
	assert_false(obs_data_has_user_value(incoming, "preserved"));
	assert_int_equal(obs_data_get_int(incoming, "overwrite"), 20);

	obs_data_set_int(incoming, "overwrite", 30);
	observation.returned = false;
	obs_output_update(&output, incoming);
	observation.returned = true;
	assert_int_equal(observation.calls, 2);
	assert_int_equal(observation.values[0], 20);
	assert_int_equal(observation.values[1], 30);
	assert_int_equal(obs_data_get_int(stored, "overwrite"), 30);
	/* Applying settings copies values: later incoming mutations do not alias them. */
	obs_data_set_int(incoming, "overwrite", 40);
	assert_int_equal(obs_data_get_int(stored, "overwrite"), 30);
	obs_data_release(incoming);
	obs_data_release(stored);
}

static void merge_without_callback(void **state)
{
	(void)state;
	obs_data_t *stored = obs_data_create();
	obs_data_t *incoming = obs_data_create();
	obs_data_set_int(stored, "overwrite", 1);
	obs_data_set_string(stored, "preserved", "original");
	obs_data_set_int(incoming, "overwrite", 2);
	obs_data_set_bool(incoming, "added", true);
	struct obs_output output = {0};
	output.context.settings = stored;
	obs_output_update(&output, incoming);
	assert_ptr_equal(output.context.settings, stored);
	assert_int_equal(obs_data_get_int(stored, "overwrite"), 2);
	assert_string_equal(obs_data_get_string(stored, "preserved"), "original");
	assert_true(obs_data_get_bool(stored, "added"));
	obs_data_release(incoming);
	obs_data_release(stored);
}

static void null_settings_and_invalid_flag(void **state)
{
	(void)state;
	obs_data_t *stored = obs_data_create();
	obs_data_set_int(stored, "overwrite", 7);
	obs_data_set_string(stored, "preserved", "original");
	obs_data_set_bool(stored, "added", true);
	struct obs_output output = {0};
	output.context.settings = stored;
	output.info.update = observe_update;
	observation = (struct update_observation){.data = NULL, .settings = stored};
	/* Characterized, not endorsed: the validity guard checks only the pointer.
	 * A false output.valid and NULL plugin context still reach the callback.
	 * NULL incoming settings are a no-op merge, followed by the callback. */
	assert_false(output.valid);
	obs_output_update(&output, NULL);
	observation.returned = true;
	assert_int_equal(observation.calls, 1);
	assert_int_equal(observation.values[0], 7);
	assert_ptr_equal(output.context.settings, stored);
	obs_data_release(stored);
}

struct update_log {
	unsigned calls;
	int level;
	char message[128];
};

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_update_log(int level, const char *format, va_list args, void *param)
{
	struct update_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void null_output(void **state)
{
	(void)state;
	obs_data_t *incoming = obs_data_create();
	obs_data_set_int(incoming, "overwrite", 19);
	struct update_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_update_log, &log);
	obs_output_update(NULL, incoming);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_output_update: Null 'output' parameter");
	assert_int_equal(obs_data_get_int(incoming, "overwrite"), 19);
	obs_data_release(incoming);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(merge_before_callback),
					   cmocka_unit_test(merge_without_callback),
					   cmocka_unit_test(null_settings_and_invalid_flag),
					   cmocka_unit_test(null_output)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
