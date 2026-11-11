/* Source type theme-icon characterization before the Rust swap (Refs #4695).
 * Exercises actual libobs exports and public registration, with no instances.
 * NULL IDs are characterized only with an empty initialized registry: the
 * existing lookup calls strcmp on IDs when populated (characterized, not
 * endorsed). Calls without an initialized core are outside the C contract. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include <obs-module.h>

static unsigned dark_calls, light_calls, name_calls;
static void *dark_data, *light_data;
static const char *dark_result, *light_result;

static const char *record_dark(void *data)
{
	++dark_calls;
	dark_data = data;
	return dark_result;
}

static const char *record_light(void *data)
{
	++light_calls;
	light_data = data;
	return light_result;
}

static const char *source_name(void *data)
{
	(void)data;
	++name_calls;
	return "Theme icon fixture";
}

static void *unused_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	assert_true(false); /* Type queries must not create an instance/get its data. */
	return NULL;
}

static void unused_destroy(void *data)
{
	(void)data;
	assert_true(false);
}

static void register_type(struct obs_source_info info)
{
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.get_name = source_name;
	info.create = unused_create;
	info.destroy = unused_destroy;
	obs_register_source(&info);
}

static int setup(void **state)
{
	(void)state;
	dark_calls = light_calls = name_calls = 0;
	dark_data = light_data = NULL;
	dark_result = light_result = NULL;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	/* Registry strings are owned by OBS; fixture tokens/results are static,
	 * and no free_type_data callback or source instance owns them. */
	obs_shutdown();
	return 0;
}

static void expect_missing(const char *id)
{
	unsigned dark_before = dark_calls, light_before = light_calls;
	assert_null(obs_source_get_dark_icon(id));
	assert_null(obs_source_get_light_icon(id));
	assert_int_equal(dark_calls, dark_before);
	assert_int_equal(light_calls, light_before);
	assert_int_equal(name_calls, 0);
}

/* Startup logs synchronously after the core exists and before built-in source
 * registration. Observe that public empty-registry state rather than passing
 * NULL to strcmp after obs_startup has registered scene/group/audio_line. */
struct startup_probe {
	log_handler_t previous;
	void *previous_param;
	bool ran;
	const char *dark, *light;
};

#if defined(__GNUC__) || defined(__clang__)
static void probe_startup_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void probe_startup_log(int level, const char *format, va_list args, void *param)
{
	struct startup_probe *probe = param;
	const char *id = NULL;
	if (!probe->ran && obs_initialized() && !obs_enum_source_types(0, &id)) {
		probe->ran = true;
		probe->dark = obs_source_get_dark_icon(NULL);
		probe->light = obs_source_get_light_icon(NULL);
	}
	if (probe->previous)
		probe->previous(level, format, args, probe->previous_param);
}

static void null_ids_during_empty_registry(void **state)
{
	(void)state;
	static const char sentinel[] = "not called";
	struct startup_probe probe = {.dark = sentinel, .light = sentinel};
	base_get_log_handler(&probe.previous, &probe.previous_param);
	base_set_log_handler(probe_startup_log, &probe);
	bool started = obs_startup("en-US", NULL, NULL);
	base_set_log_handler(probe.previous, probe.previous_param);
	if (started)
		obs_shutdown();
	assert_true(started);
	assert_true(probe.ran);
	assert_null(probe.dark);
	assert_null(probe.light);
}

static void unknown_ids(void **state)
{
	(void)state;
	expect_missing("");
	expect_missing("unregistered");
	register_type((struct obs_source_info){.id = "known", .icon_type = OBS_ICON_TYPE_CUSTOM});
	expect_missing("known");
	expect_missing("unknown");
	expect_missing("");
}

static void noncustom_skips_callbacks(void **state)
{
	(void)state;
	static char ids[OBS_ICON_TYPE_PROCESS_AUDIO_OUTPUT + 1][32];
	static unsigned token;
	for (unsigned i = 0; i <= OBS_ICON_TYPE_PROCESS_AUDIO_OUTPUT; ++i) {
		if (i == OBS_ICON_TYPE_CUSTOM)
			continue;
		snprintf(ids[i], sizeof(ids[i]), "noncustom-%u", i);
		register_type((struct obs_source_info){.id = ids[i],
						       .icon_type = (enum obs_icon_type)i,
						       .get_dark_icon = record_dark,
						       .get_light_icon = record_light,
						       .type_data = &token});
		expect_missing(ids[i]);
	}
}

static void optional_callbacks_are_independent(void **state)
{
	(void)state;
	static unsigned dark_token, light_token;
	static char dark[] = "dark.svg", light[] = "light.svg";
	dark_result = dark;
	light_result = light;
	register_type((struct obs_source_info){.id = "dark-only",
					       .icon_type = OBS_ICON_TYPE_CUSTOM,
					       .get_dark_icon = record_dark,
					       .type_data = &dark_token});
	register_type((struct obs_source_info){.id = "light-only",
					       .icon_type = OBS_ICON_TYPE_CUSTOM,
					       .get_light_icon = record_light,
					       .type_data = &light_token});
	assert_ptr_equal(obs_source_get_dark_icon("dark-only"), dark);
	assert_ptr_equal(dark_data, &dark_token);
	assert_null(obs_source_get_light_icon("dark-only"));
	assert_int_equal(dark_calls, 1);
	assert_int_equal(light_calls, 0);
	assert_null(obs_source_get_dark_icon("light-only"));
	assert_ptr_equal(obs_source_get_light_icon("light-only"), light);
	assert_ptr_equal(light_data, &light_token);
	assert_int_equal(dark_calls, 1);
	assert_int_equal(light_calls, 1);
	assert_int_equal(name_calls, 0);
}

static void exact_type_data_and_borrowed_results(void **state)
{
	(void)state;
	static unsigned token;
	static char empty[] = "", first[] = "../Mixed Case.svg", same_text[] = "../Mixed Case.svg";
	static const unsigned char bytes[] = {0xfe, 0x81, 0};
	register_type((struct obs_source_info){.id = "with-data",
					       .icon_type = OBS_ICON_TYPE_CUSTOM,
					       .get_dark_icon = record_dark,
					       .get_light_icon = record_light,
					       .type_data = &token});
	register_type((struct obs_source_info){.id = "null-data",
					       .icon_type = OBS_ICON_TYPE_CUSTOM,
					       .get_dark_icon = record_dark,
					       .get_light_icon = record_light});
	const char *results[] = {NULL, empty, first, same_text, (const char *)bytes};
	const char *ids[] = {"with-data", "null-data"};
	for (size_t j = 0; j < 2; ++j) {
		for (size_t i = 0; i < 5; ++i) {
			dark_result = results[i];
			light_result = results[4 - i];
			unsigned dark_before = dark_calls, light_before = light_calls;
			assert_ptr_equal(obs_source_get_dark_icon(ids[j]), dark_result);
			assert_int_equal(dark_calls, dark_before + 1);
			assert_int_equal(light_calls, light_before);
			assert_ptr_equal(dark_data, j == 0 ? &token : NULL);
			assert_ptr_equal(obs_source_get_light_icon(ids[j]), light_result);
			assert_int_equal(light_calls, light_before + 1);
			assert_int_equal(dark_calls, dark_before + 1);
			assert_ptr_equal(light_data, j == 0 ? &token : NULL);
		}
	}
	assert_ptr_not_equal(first, same_text);
	assert_string_equal(first, same_text);
	dark_result = light_result = first;
	const char *borrowed_dark = obs_source_get_dark_icon("with-data");
	const char *borrowed_light = obs_source_get_light_icon("with-data");
	first[0] = 'X';
	assert_int_equal(borrowed_dark[0], 'X');
	assert_int_equal(borrowed_light[0], 'X');
	first[0] = '.';
	assert_int_equal(name_calls, 0);
}

static void first_registration_and_exact_ids(void **state)
{
	(void)state;
	static unsigned first_token, second_token;
	static char result[] = "first.svg";
	struct obs_source_info info = {.id = "first",
				       .icon_type = OBS_ICON_TYPE_CUSTOM,
				       .get_dark_icon = record_dark,
				       .get_light_icon = record_light,
				       .type_data = &first_token};
	register_type(info);
	info.type_data = &second_token;
	info.icon_type = OBS_ICON_TYPE_UNKNOWN;
	register_type(info); /* Rejected duplicate must leave the original intact. */
	info.id = "first-suffix";
	register_type(info);
	dark_result = light_result = result;
	char equal_id[] = "first\0ignored";
	assert_ptr_equal(obs_source_get_dark_icon(equal_id), result);
	assert_ptr_equal(obs_source_get_light_icon(equal_id), result);
	assert_ptr_equal(dark_data, &first_token);
	assert_ptr_equal(light_data, &first_token);
	expect_missing("First");
	expect_missing("firs");
	expect_missing("first-suffix");
	info.id = "";
	info.icon_type = OBS_ICON_TYPE_CUSTOM;
	register_type(info);
	assert_ptr_equal(obs_source_get_dark_icon(""), result);
	assert_ptr_equal(obs_source_get_light_icon(""), result);
	assert_ptr_equal(dark_data, &second_token);
	assert_ptr_equal(light_data, &second_token);
	assert_int_equal(dark_calls, 2);
	assert_int_equal(light_calls, 2);
	assert_int_equal(name_calls, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_ids_during_empty_registry),
		cmocka_unit_test_setup_teardown(unknown_ids, setup, teardown),
		cmocka_unit_test_setup_teardown(noncustom_skips_callbacks, setup, teardown),
		cmocka_unit_test_setup_teardown(optional_callbacks_are_independent, setup, teardown),
		cmocka_unit_test_setup_teardown(exact_type_data_and_borrowed_results, setup, teardown),
		cmocka_unit_test_setup_teardown(first_registration_and_exact_ids, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
