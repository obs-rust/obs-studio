/* Characterize obs_get_source_defaults() before its Rust swap (Refs #5640):
 * which defaults callback runs, with what, how often, and what the caller
 * gets back. A NULL ID is left out: the type lookup compares it with strcmp
 * once any type is registered (test_source_info_lookup). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include <obs.h>

#if !__has_include("obs-source-defaults.c.h")
#error "source defaults extraction seam is missing"
#endif

static unsigned log_count;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	log_count++;
}

static unsigned legacy_calls;
static unsigned modern_calls;
static obs_data_t *legacy_settings;
static obs_data_t *modern_settings;
static void *modern_type_data;
static bool empty_at_call;

static int type_data_marker;

static void reset(void)
{
	log_count = 0;
	legacy_calls = 0;
	modern_calls = 0;
	legacy_settings = NULL;
	modern_settings = NULL;
	modern_type_data = NULL;
	empty_at_call = false;
}

static bool is_empty(obs_data_t *settings)
{
	obs_data_item_t *item = obs_data_first(settings);
	if (!item)
		return true;
	obs_data_item_release(&item);
	return false;
}

static void legacy_defaults(obs_data_t *settings)
{
	legacy_calls++;
	legacy_settings = settings;
	empty_at_call = is_empty(settings);
	obs_data_set_default_int(settings, "which", 1);
}

static void modern_defaults(void *type_data, obs_data_t *settings)
{
	modern_calls++;
	modern_type_data = type_data;
	modern_settings = settings;
	empty_at_call = is_empty(settings);
	obs_data_set_default_int(settings, "which", 2);
}

static const char *type_name(void *unused)
{
	(void)unused;
	return "Defaults";
}

static void *type_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void type_destroy(void *data)
{
	(void)data;
}

static bool type_audio_render(void *data, uint64_t *timestamp, struct obs_source_audio_mix *audio, uint32_t mixers,
			      size_t channels, size_t sample_rate)
{
	(void)data;
	(void)timestamp;
	(void)audio;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	return false;
}

static void register_type(const char *id, bool legacy, bool modern)
{
	struct obs_source_info info = {
		.id = id,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = type_name,
		.create = type_create,
		.destroy = type_destroy,
		.audio_render = type_audio_render,
		.get_defaults = legacy ? legacy_defaults : NULL,
		.get_defaults2 = modern ? modern_defaults : NULL,
		.type_data = &type_data_marker,
	};
	obs_register_source(&info);
}

static obs_data_t *defaults(const char *id)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	reset();
	base_set_log_handler(capture, NULL);
	obs_data_t *settings = obs_get_source_defaults(id);
	base_set_log_handler(previous, param);
	return settings;
}

static int setup_types(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	register_type("defaults_legacy", true, false);
	register_type("defaults_modern", false, true);
	register_type("defaults_both", true, true);
	register_type("defaults_none", false, false);
	/* Every type must be registered, or the cases below test nothing. */
	static const char *const ids[] = {"defaults_legacy", "defaults_modern", "defaults_both", "defaults_none"};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++)
		if (obs_get_source_output_flags(ids[i]) != OBS_SOURCE_AUDIO)
			return -1;
	return 0;
}

static int teardown_types(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void unknown_id_returns_null(void **state)
{
	(void)state;
	assert_null(defaults("defaults_missing"));
	assert_null(defaults(""));
	assert_null(defaults("defaults_legacy "));
	assert_int_equal(legacy_calls + modern_calls, 0);
	assert_int_equal(log_count, 0);
}

static void legacy_callback_fills_returned_object(void **state)
{
	(void)state;
	obs_data_t *settings = defaults("defaults_legacy");
	assert_non_null(settings);
	assert_int_equal(legacy_calls, 1);
	assert_int_equal(modern_calls, 0);
	assert_ptr_equal(legacy_settings, settings);
	assert_true(empty_at_call);
	assert_int_equal(obs_data_get_int(settings, "which"), 1);
	assert_int_equal(log_count, 0);
	obs_data_release(settings);
}

static void modern_callback_gets_type_data(void **state)
{
	(void)state;
	obs_data_t *settings = defaults("defaults_modern");
	assert_non_null(settings);
	assert_int_equal(modern_calls, 1);
	assert_int_equal(legacy_calls, 0);
	assert_ptr_equal(modern_settings, settings);
	assert_ptr_equal(modern_type_data, &type_data_marker);
	assert_true(empty_at_call);
	assert_int_equal(obs_data_get_int(settings, "which"), 2);
	assert_int_equal(log_count, 0);
	obs_data_release(settings);
}

static void modern_callback_wins(void **state)
{
	(void)state;
	obs_data_t *settings = defaults("defaults_both");
	assert_non_null(settings);
	assert_int_equal(modern_calls, 1);
	assert_int_equal(legacy_calls, 0);
	assert_ptr_equal(modern_settings, settings);
	assert_int_equal(obs_data_get_int(settings, "which"), 2);
	obs_data_release(settings);
}

static void no_callback_returns_empty_object(void **state)
{
	(void)state;
	obs_data_t *settings = defaults("defaults_none");
	assert_non_null(settings);
	assert_int_equal(legacy_calls + modern_calls, 0);
	assert_true(is_empty(settings));
	assert_int_equal(log_count, 0);
	obs_data_release(settings);
}

/* Every call builds its own object, owned by the caller: one release each
 * frees it, and the other one stays usable. */
static void each_call_returns_a_new_object(void **state)
{
	(void)state;
	obs_data_t *first = defaults("defaults_legacy");
	obs_data_t *second = defaults("defaults_legacy");
	assert_non_null(first);
	assert_non_null(second);
	assert_ptr_not_equal(first, second);
	obs_data_set_int(first, "which", 7);
	assert_int_equal(obs_data_get_int(second, "which"), 1);
	obs_data_release(first);
	assert_int_equal(obs_data_get_int(second, "which"), 1);
	obs_data_release(second);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(unknown_id_returns_null),
		cmocka_unit_test(legacy_callback_fills_returned_object),
		cmocka_unit_test(modern_callback_gets_type_data),
		cmocka_unit_test(modern_callback_wins),
		cmocka_unit_test(no_callback_returns_empty_object),
		cmocka_unit_test(each_call_returns_a_new_object),
	};
	return cmocka_run_group_tests(tests, setup_types, teardown_types);
}
