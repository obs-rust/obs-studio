/* Source state must be ready before a plugin starts capture in create (#3505). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <util/bmem.h>

struct observed_state {
	bool enabled;
	uint32_t flags;
};

static const char *source_name(void *unused)
{
	(void)unused;
	return "create state probe";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	struct observed_state *state = bzalloc(sizeof(*state));
	state->enabled = obs_source_enabled(source);
	state->flags = obs_source_get_flags(source);
	if (obs_data_get_bool(settings, "custom_defaults"))
		obs_source_set_default_flags(source, OBS_SOURCE_FLAG_FORCE_MONO);
	return state;
}

static void source_destroy(void *data)
{
	bfree(data);
}

static struct obs_source_info info = {
	.id = "test_source_create_state",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = source_name,
	.create = source_create,
	.destroy = source_destroy,
};

static int setup(void **unused)
{
	(void)unused;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	obs_register_source(&info);
	return 0;
}

static int teardown(void **unused)
{
	(void)unused;
	obs_shutdown();
	return 0;
}

static void enabled_during_create(void **unused)
{
	(void)unused;
	obs_source_t *source = obs_source_create_private(info.id, "probe", NULL);
	assert_non_null(source);
	const struct observed_state observed = *(struct observed_state *)obs_obj_get_data(source);
	const bool enabled = obs_source_enabled(source);
	obs_source_release(source);
	obs_wait_for_destroy_queue();
	assert_true(observed.enabled);
	assert_int_equal(observed.flags, 0);
	assert_true(enabled);
}

static void custom_defaults_during_create(void **unused)
{
	(void)unused;
	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "custom_defaults", true);
	obs_source_t *source = obs_source_create_private(info.id, "custom", settings);
	obs_data_release(settings);
	assert_non_null(source);
	const uint32_t flags = obs_source_get_flags(source);
	obs_source_release(source);
	obs_wait_for_destroy_queue();
	assert_int_equal(flags, OBS_SOURCE_FLAG_FORCE_MONO);
}

static void defaults_after_create_do_not_change_flags(void **unused)
{
	(void)unused;
	obs_source_t *source = obs_source_create_private(info.id, "later", NULL);
	assert_non_null(source);
	obs_source_set_default_flags(source, OBS_SOURCE_FLAG_FORCE_MONO);
	const uint32_t flags = obs_source_get_flags(source);
	obs_source_release(source);
	obs_wait_for_destroy_queue();
	assert_int_equal(flags, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enabled_during_create),
		cmocka_unit_test(custom_defaults_during_create),
		cmocka_unit_test(defaults_after_create_do_not_change_flags),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
