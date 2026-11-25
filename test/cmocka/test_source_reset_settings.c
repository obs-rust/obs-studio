/* Characterize obs_source_reset_settings() before extraction and its later
 * Rust swap (Refs #5718). Run unchanged against hybrid libobs and
 * libobs-c-reference. Only non-video sources are used, so no video or
 * graphics context is needed: their updates dispatch synchronously. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#define INPUT_ID "test_source_reset_settings_input"

static unsigned log_count;
static int log_level;
static char log_message[128];

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

struct probe {
	obs_source_t *source;
	unsigned updates;
	obs_data_t *update_settings;
	bool saw_old;
	long long seen_new;
	char events[16];
	size_t count;
};

static struct probe probe;

static void record(char event)
{
	assert_true(probe.count + 1 < sizeof(probe.events));
	probe.events[probe.count++] = event;
	probe.events[probe.count] = '\0';
}

static const char *source_name(void *type_data)
{
	return type_data;
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *data)
{
	(void)data;
}

static void source_update(void *data, obs_data_t *settings)
{
	if (data != probe.source)
		return;
	probe.updates++;
	probe.update_settings = settings;
	/* The clear already happened when the callback sees the settings. */
	probe.saw_old = obs_data_has_user_value(settings, "old");
	probe.seen_new = obs_data_get_int(settings, "new");
	record('U');
}

static void on_signal(void *param, calldata_t *data)
{
	if (calldata_ptr(data, "source") == probe.source)
		record(*(const char *)param);
}

static const char global_event = 'G';
static const char local_event = 'L';

static obs_source_t *create_source(void)
{
	obs_data_t *initial = obs_data_create();
	obs_data_set_int(initial, "old", 7);
	obs_data_set_int(initial, "new", 1);
	obs_source_t *source = obs_source_create(INPUT_ID, INPUT_ID, initial, NULL);
	obs_data_release(initial);
	assert_non_null(source);

	memset(&probe, 0, sizeof(probe));
	probe.source = source;
	probe.seen_new = -1;
	signal_handler_connect(obs_get_signal_handler(), "source_update", on_signal, (void *)&global_event);
	signal_handler_connect(obs_source_get_signal_handler(source), "update", on_signal, (void *)&local_event);
	return source;
}

static void destroy_source(obs_source_t *source)
{
	signal_handler_disconnect(obs_source_get_signal_handler(source), "update", on_signal, (void *)&local_event);
	signal_handler_disconnect(obs_get_signal_handler(), "source_update", on_signal, (void *)&global_event);
	probe.source = NULL;
	obs_source_release(source);
}

static void null_source_logs_the_caller(void **state)
{
	(void)state;
	log_handler_t saved_handler;
	void *saved_param;
	obs_data_t *settings = obs_data_create();
	base_get_log_handler(&saved_handler, &saved_param);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
	base_set_log_handler(capture_log, NULL);
	obs_source_reset_settings(NULL, settings);
	base_set_log_handler(saved_handler, saved_param);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_reset_settings: Null 'source' parameter");
	obs_data_release(settings);
}

static void clears_before_update(void **state)
{
	(void)state;
	obs_source_t *source = create_source();
	obs_data_t *own = obs_source_get_settings(source);
	obs_data_t *replacement = obs_data_create();
	obs_data_set_int(replacement, "new", 42);

	obs_source_reset_settings(source, replacement);

	/* The source keeps its own settings object; only its contents change. */
	obs_data_t *after = obs_source_get_settings(source);
	assert_ptr_equal(after, own);
	assert_false(obs_data_has_user_value(after, "old"));
	assert_int_equal(obs_data_get_int(after, "new"), 42);
	/* The replacement is applied by copy, not adopted or modified. */
	assert_int_equal(obs_data_get_int(replacement, "new"), 42);
	assert_false(obs_data_has_user_value(replacement, "old"));

	assert_int_equal(probe.updates, 1);
	assert_ptr_equal(probe.update_settings, own);
	assert_false(probe.saw_old);
	assert_int_equal(probe.seen_new, 42);
	assert_string_equal(probe.events, "UGL");

	obs_data_release(after);
	obs_data_release(own);
	obs_data_release(replacement);
	destroy_source(source);
}

static void null_settings_clears_and_still_updates(void **state)
{
	(void)state;
	obs_source_t *source = create_source();

	obs_source_reset_settings(source, NULL);

	obs_data_t *after = obs_source_get_settings(source);
	assert_false(obs_data_has_user_value(after, "old"));
	assert_false(obs_data_has_user_value(after, "new"));
	assert_int_equal(probe.updates, 1);
	assert_false(probe.saw_old);
	assert_int_equal(probe.seen_new, 0);
	assert_string_equal(probe.events, "UGL");

	obs_data_release(after);
	destroy_source(source);
}

static void own_settings_alias_ends_empty(void **state)
{
	(void)state;
	obs_source_t *source = create_source();
	obs_data_t *own = obs_source_get_settings(source);

	/* Characterized, not endorsed: the clear empties the aliased input
	 * before the merge, so nothing survives. */
	obs_source_reset_settings(source, own);

	assert_false(obs_data_has_user_value(own, "old"));
	assert_false(obs_data_has_user_value(own, "new"));
	assert_int_equal(probe.updates, 1);
	assert_false(probe.saw_old);
	assert_int_equal(probe.seen_new, 0);
	assert_string_equal(probe.events, "UGL");

	obs_data_release(own);
	destroy_source(source);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	struct obs_source_info info = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
		.update = source_update,
		.type_data = (void *)INPUT_ID,
	};
	obs_register_source(&info);
	if (!obs_source_get_display_name(INPUT_ID)) {
		obs_shutdown();
		return 1;
	}
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source_logs_the_caller),
		cmocka_unit_test(clears_before_update),
		cmocka_unit_test(null_settings_clears_and_still_updates),
		cmocka_unit_test(own_settings_alias_ends_empty),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
