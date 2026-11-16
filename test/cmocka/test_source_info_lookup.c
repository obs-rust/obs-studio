/* Public-API characterization of the source type lookup (get_source_info in
 * obs-source-info-lookup.c.h) before its port: every call below reaches the
 * lookup with the given ID and reports what it found. A NULL ID is left out:
 * the C compares it with strcmp once any type is registered. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>

static const char *lookup_first_name(void *unused)
{
	(void)unused;
	return "Lookup first";
}

static const char *lookup_second_name(void *unused)
{
	(void)unused;
	return "Lookup second";
}

static void *lookup_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void lookup_destroy(void *data)
{
	(void)data;
}

static obs_properties_t *lookup_properties(void *data)
{
	(void)data;
	return obs_properties_create();
}

static void lookup_first_defaults(obs_data_t *settings)
{
	obs_data_set_default_int(settings, "which", 1);
}

static void lookup_second_defaults(obs_data_t *settings)
{
	obs_data_set_default_int(settings, "which", 2);
}

static bool lookup_audio_render(void *data, uint64_t *timestamp, struct obs_source_audio_mix *audio, uint32_t mixers,
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

#define FIRST_FLAGS (OBS_SOURCE_AUDIO)
#define SECOND_FLAGS (OBS_SOURCE_AUDIO | OBS_SOURCE_DO_NOT_DUPLICATE)

static void register_first(const char *id, uint32_t version)
{
	struct obs_source_info info = {
		.id = id,
		.version = version,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = FIRST_FLAGS,
		.get_name = lookup_first_name,
		.create = lookup_create,
		.destroy = lookup_destroy,
		.get_defaults = lookup_first_defaults,
		.get_properties = lookup_properties,
		.audio_render = lookup_audio_render,
	};
	obs_register_source(&info);
}

static void register_second(const char *id, uint32_t version)
{
	struct obs_source_info info = {
		.id = id,
		.version = version,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = SECOND_FLAGS,
		.get_name = lookup_second_name,
		.create = lookup_create,
		.destroy = lookup_destroy,
		.get_defaults = lookup_second_defaults,
		.audio_render = lookup_audio_render,
	};
	obs_register_source(&info);
}

/* What each public call reports for an ID the lookup does not find. */
static void assert_not_found(const char *id)
{
	assert_int_equal(obs_get_source_output_flags(id), 0);
	assert_null(obs_source_get_display_name(id));
	assert_false(obs_is_source_configurable(id));
	assert_null(obs_get_source_defaults(id));
	assert_null(obs_get_source_properties(id));
	assert_int_equal(obs_source_get_icon_type(id), OBS_ICON_TYPE_UNKNOWN);
}

static void assert_found(const char *id, uint32_t flags, const char *name, bool configurable, long long which)
{
	assert_int_equal(obs_get_source_output_flags(id), flags);
	assert_string_equal(obs_source_get_display_name(id), name);
	assert_int_equal(obs_is_source_configurable(id), configurable);

	obs_data_t *defaults = obs_get_source_defaults(id);
	assert_non_null(defaults);
	assert_int_equal(obs_data_get_int(defaults, "which"), which);
	obs_data_release(defaults);
}

static void unknown_ids_before_registration(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	assert_not_found("lookup_first");
	assert_not_found("");
	obs_shutdown();
}

static void registered_ids_and_near_misses(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	register_first("lookup_first", 0);
	register_second("lookup_second", 0);

	assert_found("lookup_first", FIRST_FLAGS, "Lookup first", true, 1);
	assert_found("lookup_second", SECOND_FLAGS, "Lookup second", false, 2);

	/* The properties call goes through the same lookup. */
	obs_properties_t *props = obs_get_source_properties("lookup_first");
	assert_non_null(props);
	obs_properties_destroy(props);
	assert_null(obs_get_source_properties("lookup_second"));

	/* Whole-string, case-sensitive compare: no prefix, suffix or case match. */
	const char *misses[] = {"",
				"lookup",
				"lookup_firs",
				"lookup_first ",
				"lookup_firstx",
				"LOOKUP_FIRST",
				"Lookup_first",
				"lookup_third"};
	for (size_t i = 0; i < sizeof(misses) / sizeof(misses[0]); i++)
		assert_not_found(misses[i]);
	obs_shutdown();
}

/* Two types can end up with the same full ID: "lookup_dup" at version 1 is
 * stored as "lookup_dup_v1", the same ID as an unversioned "lookup_dup_v1".
 * The registration's duplicate check compares unversioned IDs, so both are
 * kept, and the lookup returns the one registered first. */
static void first_registered_match_wins(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	register_first("lookup_dup_v1", 0);
	register_second("lookup_dup", 1);
	assert_found("lookup_dup_v1", FIRST_FLAGS, "Lookup first", true, 1);
	obs_shutdown();

	assert_true(obs_startup("en-US", NULL, NULL));
	register_second("lookup_dup", 1);
	register_first("lookup_dup_v1", 0);
	assert_found("lookup_dup_v1", SECOND_FLAGS, "Lookup second", false, 2);
	/* The unversioned ID of a versioned type is not its lookup ID. */
	assert_not_found("lookup_dup");
	obs_shutdown();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(unknown_ids_before_registration),
		cmocka_unit_test(registered_ids_and_near_misses),
		cmocka_unit_test(first_registered_match_wins),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
