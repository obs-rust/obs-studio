/* Source-type metadata characterization before the Rust swap (Refs #4718).
 * Exercises actual libobs exports and public registration, with no instances.
 * NULL IDs are valid only with a live empty registry: the C lookup otherwise
 * passes them to strcmp (characterized, not endorsed). No-core calls and a
 * found record without get_name are outside the valid domain. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include <obs-module.h>

static unsigned name_calls;
static void *last_type_data;
static const char *next_name;

static const char *record_name(void *type_data)
{
	++name_calls;
	last_type_data = type_data;
	return next_name;
}

static const char *rejected_name(void *type_data)
{
	(void)type_data;
	assert_true(false);
	return NULL;
}

static void *unused_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	assert_true(false);
	return NULL;
}

static void unused_destroy(void *data)
{
	(void)data;
	assert_true(false);
}

static uint32_t unused_size(void *data)
{
	(void)data;
	assert_true(false);
	return 0;
}

static bool unused_audio_render(void *data, uint64_t *ts, struct obs_source_audio_mix *mix, uint32_t mixers,
				size_t channels, size_t sample_rate)
{
	(void)data;
	(void)ts;
	(void)mix;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	assert_true(false);
	return false;
}

static obs_properties_t *unused_properties(void *data)
{
	(void)data;
	assert_true(false);
	return NULL;
}

static obs_properties_t *unused_properties2(void *data, void *type_data)
{
	(void)data;
	(void)type_data;
	assert_true(false);
	return NULL;
}

static void unused_defaults(obs_data_t *settings)
{
	(void)settings;
	assert_true(false);
}

static void unused_defaults2(void *type_data, obs_data_t *settings)
{
	(void)type_data;
	(void)settings;
	assert_true(false);
}

static void register_type(struct obs_source_info info)
{
	info.type = OBS_SOURCE_TYPE_INPUT;
	if (!info.get_name)
		info.get_name = record_name;
	info.create = unused_create;
	info.destroy = unused_destroy;
	info.get_width = unused_size;
	info.get_height = unused_size;
	info.audio_render = unused_audio_render;
	obs_register_source(&info);
}

static int setup(void **state)
{
	(void)state;
	name_calls = 0;
	last_type_data = NULL;
	next_name = NULL;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	/* OBS owns and frees the registry/ID copies. Original IDs/results/type_data
	 * have static lifetime, no free_type_data callback and no instances. */
	obs_shutdown();
	return 0;
}

static void expect_missing(const char *id)
{
	unsigned before = name_calls;
	assert_null(obs_source_get_display_name(id));
	assert_int_equal(obs_get_source_output_flags(id), 0);
	assert_false(obs_is_source_configurable(id));
	assert_int_equal(name_calls, before);
}

/* Startup logs after the core exists but before built-in sources are added.
 * Verify that empty state through public enumeration before querying NULL. */
struct startup_probe {
	log_handler_t previous;
	void *previous_param;
	bool ran;
	const char *name;
	uint32_t flags;
	bool configurable;
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
		probe->name = obs_source_get_display_name(NULL);
		probe->flags = obs_get_source_output_flags(NULL);
		probe->configurable = obs_is_source_configurable(NULL);
		expect_missing("");
		expect_missing("unregistered-source-type");
	}
	if (probe->previous)
		probe->previous(level, format, args, probe->previous_param);
}

static void null_ids_during_empty_registry(void **state)
{
	(void)state;
	struct startup_probe probe = {.name = "not called", .flags = UINT32_MAX, .configurable = true};
	base_get_log_handler(&probe.previous, &probe.previous_param);
	base_set_log_handler(probe_startup_log, &probe);
	bool started = obs_startup("en-US", NULL, NULL);
	base_set_log_handler(probe.previous, probe.previous_param);
	if (started)
		obs_shutdown();
	assert_true(started);
	assert_true(probe.ran);
	assert_null(probe.name);
	assert_int_equal(probe.flags, 0);
	assert_false(probe.configurable);
}

static void unknown_ids(void **state)
{
	(void)state;
	expect_missing("");
	expect_missing("unregistered-source-type");
	register_type((struct obs_source_info){.id = "known"});
	expect_missing("");
	expect_missing("unknown");
	expect_missing("Known");
	expect_missing("know");
	expect_missing("known-suffix");
}

static void exact_name_callback_and_borrowed_result(void **state)
{
	(void)state;
	static unsigned token;
	static char empty[] = "", first[] = "Mixed Name", same_text[] = "Mixed Name";
	static const unsigned char raw[] = {0xfe, 0x81, 0};
	register_type((struct obs_source_info){.id = "with-data", .type_data = &token});
	register_type((struct obs_source_info){.id = "null-data"});
	const char *ids[] = {"with-data", "null-data"};
	const char *results[] = {NULL, empty, first, same_text, (const char *)raw};
	for (size_t j = 0; j < 2; ++j) {
		for (size_t i = 0; i < 5; ++i) {
			next_name = results[i];
			unsigned before = name_calls;
			assert_ptr_equal(obs_source_get_display_name(ids[j]), results[i]);
			assert_int_equal(name_calls, before + 1);
			assert_ptr_equal(last_type_data, j == 0 ? &token : NULL);
			assert_int_equal(obs_get_source_output_flags(ids[j]), 0);
			assert_false(obs_is_source_configurable(ids[j]));
			assert_int_equal(name_calls, before + 1);
		}
	}
	assert_ptr_not_equal(first, same_text);
	assert_string_equal(first, same_text);
	next_name = first;
	const char *borrowed = obs_source_get_display_name("with-data");
	first[0] = 'X';
	assert_int_equal(borrowed[0], 'X');
	first[0] = 'M';
}

static void exact_flags(void **state)
{
	(void)state;
	/* INPUT registration preserves flags. Individual bits include reserved
	 * and high bits; composite+audio/async combinations are rejected by
	 * registration, so the mixed all-bits fixture omits only composite. */
	static char ids[32][64];
	for (unsigned bit = 0; bit < 32; ++bit) {
		const char *id = ids[bit];
		snprintf(ids[bit], sizeof(ids[bit]), "source-flag-%u", bit);
		uint32_t flags = UINT32_C(1) << bit;
		register_type((struct obs_source_info){.id = id, .output_flags = flags});
		assert_int_equal(obs_get_source_output_flags(id), flags);
	}
	uint32_t mixed = UINT32_MAX ^ OBS_SOURCE_COMPOSITE;
	register_type((struct obs_source_info){.id = "mixed-flags", .output_flags = mixed});
	register_type((struct obs_source_info){.id = "zero-flags"});
	assert_int_equal(obs_get_source_output_flags("mixed-flags"), mixed);
	assert_int_equal(obs_get_source_output_flags("zero-flags"), 0);
	assert_int_equal(name_calls, 0);
}

static void configurable_callback_presence(void **state)
{
	(void)state;
	static char ids[16][64];
	/* Cover both properties callbacks crossed with both defaults callbacks.
	 * No callback may run: defaults alone do not imply configurability. */
	for (unsigned mask = 0; mask < 16; ++mask) {
		const char *id = ids[mask];
		snprintf(ids[mask], sizeof(ids[mask]), "source-configurable-%u", mask);
		struct obs_source_info info = {.id = id,
					       .get_properties = (mask & 1) ? unused_properties : NULL,
					       .get_properties2 = (mask & 2) ? unused_properties2 : NULL,
					       .get_defaults = (mask & 4) ? unused_defaults : NULL,
					       .get_defaults2 = (mask & 8) ? unused_defaults2 : NULL};
		register_type(info);
		assert_int_equal(obs_is_source_configurable(id), (mask & 3) != 0);
		assert_int_equal(obs_get_source_output_flags(id), 0);
	}
	assert_int_equal(name_calls, 0);
}

static void first_registration_and_exact_ids(void **state)
{
	(void)state;
	static unsigned first_token, second_token;
	static char result[] = "first name";
	struct obs_source_info info = {.id = "first",
				       .type_data = &first_token,
				       .output_flags = OBS_SOURCE_AUDIO,
				       .get_properties2 = unused_properties2};
	register_type(info);
	info.type_data = &second_token;
	info.get_name = rejected_name;
	info.output_flags = 0;
	info.get_properties2 = NULL;
	register_type(info); /* Rejected duplicate retains all original metadata. */
	next_name = result;
	char equal_id[] = "first\0ignored";
	assert_ptr_equal(obs_source_get_display_name(equal_id), result);
	assert_ptr_equal(last_type_data, &first_token);
	assert_int_equal(name_calls, 1);
	assert_int_equal(obs_get_source_output_flags(equal_id), OBS_SOURCE_AUDIO);
	assert_true(obs_is_source_configurable(equal_id));
	expect_missing("First");
	expect_missing("firs");
	expect_missing("first-suffix");
	static const unsigned char raw_id[] = {0xfe, 'i', 'd', 0};
	const char *ids[] = {"", "first-suffix", (const char *)raw_id};
	for (size_t i = 0; i < 3; ++i) {
		info.id = ids[i];
		info.get_name = record_name;
		info.output_flags = UINT32_C(0x80000000);
		info.get_properties = unused_properties;
		register_type(info);
		assert_ptr_equal(obs_source_get_display_name(ids[i]), result);
		assert_ptr_equal(last_type_data, &second_token);
		assert_int_equal(obs_get_source_output_flags(ids[i]), UINT32_C(0x80000000));
		assert_true(obs_is_source_configurable(ids[i]));
	}
	assert_int_equal(obs_get_source_output_flags("first"), OBS_SOURCE_AUDIO);
	assert_int_equal(name_calls, 4);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_ids_during_empty_registry),
		cmocka_unit_test_setup_teardown(unknown_ids, setup, teardown),
		cmocka_unit_test_setup_teardown(exact_name_callback_and_borrowed_result, setup, teardown),
		cmocka_unit_test_setup_teardown(exact_flags, setup, teardown),
		cmocka_unit_test_setup_teardown(configurable_callback_presence, setup, teardown),
		cmocka_unit_test_setup_teardown(first_registration_and_exact_ids, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
