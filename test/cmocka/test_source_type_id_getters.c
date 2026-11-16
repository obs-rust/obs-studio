/* Public-API characterization before extracting source identity getters (#4509).
 * The ID is borrowed type identity, independent of the mutable context name. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>

static const char *identity_name(void *unused)
{
	(void)unused;
	return "Identity getter characterization";
}

static void *identity_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void identity_destroy(void *data)
{
	(void)data;
}

static bool identity_audio_render(void *data, uint64_t *timestamp, struct obs_source_audio_mix *audio, uint32_t mixers,
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

static void null_defaults(void **state)
{
	(void)state;
	assert_int_equal(obs_source_get_type(NULL), OBS_SOURCE_TYPE_INPUT);
	assert_null(obs_source_get_id(NULL));
}

static void registered_identity(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	const enum obs_source_type types[] = {OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_TYPE_FILTER,
					      OBS_SOURCE_TYPE_TRANSITION};
	const char *ids[] = {"identity_input", "identity_filter", "identity_transition"};
	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		struct obs_source_info info = {
			.id = ids[i],
			.type = types[i],
			.get_name = identity_name,
			.create = identity_create,
			.destroy = identity_destroy,
			.audio_render = identity_audio_render,
		};
		obs_register_source(&info);
		obs_source_t *source = obs_source_create_private(ids[i], "original context name", NULL);
		assert_non_null(source);
		assert_int_equal(obs_source_get_type(source), types[i]);
		const char *borrowed_id = obs_source_get_id(source);
		assert_non_null(borrowed_id);
		assert_string_equal(borrowed_id, ids[i]);
		assert_ptr_equal(obs_source_get_id(source), borrowed_id);
		obs_source_set_name(source, "renamed context");
		assert_int_equal(obs_source_get_type(source), types[i]);
		assert_ptr_equal(obs_source_get_id(source), borrowed_id);
		assert_string_equal(borrowed_id, ids[i]);
		obs_source_release(source);
		obs_wait_for_destroy_queue();
	}
	obs_shutdown();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_defaults),
		cmocka_unit_test(registered_identity),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
