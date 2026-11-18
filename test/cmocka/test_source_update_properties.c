/* Characterize obs_source_update_properties() before its later Rust swap (Refs #5126).
 * Run unchanged against hybrid libobs and libobs-c-reference. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-source-update-properties.c.h")
#error "source property-refresh notification extraction seam is missing"
#endif

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

struct signal_probe {
	unsigned source_calls;
	obs_source_t *source_arg;
	unsigned global_calls;
};

static void on_update_properties(void *data, calldata_t *cd)
{
	struct signal_probe *probe = data;
	probe->source_calls++;
	probe->source_arg = calldata_ptr(cd, "source");
}

static void on_any_global(void *data, const char *name, calldata_t *cd)
{
	(void)name;
	(void)cd;
	struct signal_probe *probe = data;
	probe->global_calls++;
}

/* Calls obs_source_update_properties() with logging captured. */
static void update_properties(obs_source_t *source)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	obs_source_update_properties(source);
	base_set_log_handler(old_handler, old_param);
}

static void null_source(void **state)
{
	(void)state;
	log_count = 0;
	update_properties(NULL);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_update_properties: Null 'source' parameter");
}

/* Each call emits the source's own "update_properties" signal once, with the
 * source as its "source" argument, and nothing on the core handler, whether
 * or not the source is private. */
static void signals_source_only(bool private_scene)
{
	obs_scene_t *scene = private_scene ? obs_scene_create_private("update properties private")
					   : obs_scene_create("update properties public");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	struct signal_probe probe = {0};
	signal_handler_t *source_signals = obs_source_get_signal_handler(source);
	signal_handler_t *core_signals = obs_get_signal_handler();
	signal_handler_connect(source_signals, "update_properties", on_update_properties, &probe);
	signal_handler_connect_global(core_signals, on_any_global, &probe);

	log_count = 0;
	for (unsigned i = 1; i <= 3; i++) {
		update_properties(source);
		assert_int_equal(probe.source_calls, i);
		assert_ptr_equal(probe.source_arg, source);
	}
	assert_int_equal(probe.global_calls, 0);
	assert_int_equal(log_count, 0);

	signal_handler_disconnect_global(core_signals, on_any_global, &probe);
	signal_handler_disconnect(source_signals, "update_properties", on_update_properties, &probe);
	obs_scene_release(scene);
	while (obs_wait_for_destroy_queue())
		;
}

static void public_source(void **state)
{
	(void)state;
	signals_source_only(false);
}

static void private_source(void **state)
{
	(void)state;
	signals_source_only(true);
}

/* The source's settings are left alone: it only notifies. */
static void settings_untouched(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("update properties settings");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	obs_data_t *settings = obs_source_get_settings(source);
	obs_data_set_string(settings, "marker", "kept");
	char *before = bstrdup(obs_data_get_json(settings));
	update_properties(source);
	assert_string_equal(obs_data_get_json(settings), before);
	bfree(before);
	obs_data_release(settings);
	obs_scene_release(scene);
	while (obs_wait_for_destroy_queue())
		;
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test(public_source),
		cmocka_unit_test(private_source),
		cmocka_unit_test(settings_untouched),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
