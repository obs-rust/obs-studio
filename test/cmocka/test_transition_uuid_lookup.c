/* Actual-library characterization of the deprecated, exported UUID lookup.
 * Private transitions remain discoverable and successful results are owned. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>

/* The deprecated symbol is the compatibility contract under test. */
PRAGMA_DISABLE_DEPRECATION

static const char *lookup_name(void *unused)
{
	(void)unused;
	return "transition UUID lookup fixture";
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

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_source_info info = {
		.id = "uuid_lookup_input",
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = lookup_name,
		.create = lookup_create,
		.destroy = lookup_destroy,
	};
	obs_register_source(&info);
	info.id = "uuid_lookup_transition";
	info.type = OBS_SOURCE_TYPE_TRANSITION;
	/* Transitions are composite sources and require an audio callback. */
	info.audio_render = lookup_audio_render;
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return 0;
}

static void missing_uuid(void **state)
{
	(void)state;
	assert_null(obs_get_transition_by_uuid("00000000-0000-0000-0000-000000000000"));
	assert_null(obs_get_transition_by_uuid(""));
}

static void retained_transition(bool private)
{
	obs_source_t *source = private ? obs_source_create_private("uuid_lookup_transition", "transition", NULL)
				       : obs_source_create("uuid_lookup_transition", "transition", NULL, NULL);
	assert_non_null(source);
	assert_int_equal(obs_source_get_type(source), OBS_SOURCE_TYPE_TRANSITION);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_non_null(weak);
	const char *uuid = obs_source_get_uuid(source);
	assert_non_null(uuid);
	obs_source_t *first = obs_get_transition_by_uuid(uuid);
	obs_source_t *second = obs_get_transition_by_uuid(uuid);
	assert_ptr_equal(first, source);
	assert_ptr_equal(second, source);
	obs_source_release(source);
	assert_false(obs_weak_source_expired(weak));
	obs_source_release(first);
	assert_false(obs_weak_source_expired(weak));
	obs_source_release(second);
	assert_true(obs_weak_source_expired(weak));
	obs_wait_for_destroy_queue();
	obs_weak_source_release(weak);
}

static void public_transition(void **state)
{
	(void)state;
	retained_transition(false);
}

static void private_transition(void **state)
{
	(void)state;
	retained_transition(true);
}

static void rejected_input(bool private)
{
	obs_source_t *source = private ? obs_source_create_private("uuid_lookup_input", "input", NULL)
				       : obs_source_create("uuid_lookup_input", "input", NULL, NULL);
	assert_non_null(source);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_non_null(weak);
	const char *uuid = obs_source_get_uuid(source);
	/* Establish that rejection is a type check, not a missing registry entry. */
	obs_source_t *found = obs_get_source_by_uuid(uuid);
	assert_ptr_equal(found, source);
	obs_source_release(found);
	assert_null(obs_get_transition_by_uuid(uuid));
	assert_null(obs_get_transition_by_uuid(uuid));
	assert_false(obs_weak_source_expired(weak));
	obs_source_release(source);
	/* Characterized ownership: the rejected lookup releases its temporary ref. */
	assert_true(obs_weak_source_expired(weak));
	obs_wait_for_destroy_queue();
	obs_weak_source_release(weak);
}

static void public_input(void **state)
{
	(void)state;
	rejected_input(false);
}

static void private_input(void **state)
{
	(void)state;
	rejected_input(true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(missing_uuid),       cmocka_unit_test(public_transition),
		cmocka_unit_test(private_transition), cmocka_unit_test(public_input),
		cmocka_unit_test(private_input),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
