/* Characterize the public source conversions before their extraction/Rust swap.
 * This fixture runs against both actual libobs implementations. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>

static const char *fixture_name(void *unused)
{
	(void)unused;
	return "source conversion fixture";
}

static void *fixture_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	/* A non-NULL context makes an accidental conversion observable. */
	return source;
}

static void fixture_destroy(void *data)
{
	(void)data;
}

static void assert_borrowed_release(obs_source_t *source, obs_weak_source_t *weak)
{
	assert_false(obs_weak_source_expired(weak));
	obs_source_t *retained = obs_weak_source_get_source(weak);
	assert_ptr_equal(retained, source);
	obs_source_release(retained);
	/* Conversion results are borrowed: release only the original owner. */
	obs_source_release(source);
	while (obs_wait_for_destroy_queue())
		;
	assert_true(obs_weak_source_expired(weak));
	assert_null(obs_weak_source_get_source(weak));
	obs_weak_source_release(weak);
}

static void null_source(void **state)
{
	(void)state;
	assert_null(obs_scene_from_source(NULL));
	assert_null(obs_group_from_source(NULL));
}

static void non_scene_sources(void **state)
{
	(void)state;
	/* Prefix/case lookalikes must not match the registered scene/group IDs. */
	const char *ids[] = {"scene-extra", "group-extra", "Scene", "Group"};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
		struct obs_source_info info = {
			.id = ids[i],
			.type = OBS_SOURCE_TYPE_INPUT,
			.get_name = fixture_name,
			.create = fixture_create,
			.destroy = fixture_destroy,
		};
		obs_register_source(&info);
		obs_source_t *source = obs_source_create_private(ids[i], "non-scene", NULL);
		assert_non_null(source);
		assert_string_equal(obs_source_get_id(source), ids[i]);
		obs_weak_source_t *weak = obs_source_get_weak_source(source);
		assert_non_null(weak);
		for (unsigned repeat = 0; repeat < 3; repeat++) {
			assert_null(obs_scene_from_source(source));
			assert_null(obs_group_from_source(source));
			assert_false(obs_weak_source_expired(weak));
		}
		assert_borrowed_release(source, weak);
	}
}

static void scene_source(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("conversion scene");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	assert_non_null(source);
	assert_string_equal(obs_source_get_id(source), "scene");
	assert_int_equal(obs_source_get_type(source), OBS_SOURCE_TYPE_SCENE);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_non_null(weak);
	for (unsigned repeat = 0; repeat < 3; repeat++) {
		assert_ptr_equal(obs_scene_from_source(source), scene);
		assert_null(obs_group_from_source(source));
		assert_false(obs_weak_source_expired(weak));
	}
	assert_borrowed_release(source, weak);
}

static void group_source(void **state)
{
	(void)state;
	obs_source_t *source = obs_source_create_private("group", "conversion group", NULL);
	assert_non_null(source);
	assert_string_equal(obs_source_get_id(source), "group");
	/* Scene and group share a source type, but the conversions match IDs. */
	assert_int_equal(obs_source_get_type(source), OBS_SOURCE_TYPE_SCENE);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_non_null(weak);
	obs_scene_t *group = obs_group_from_source(source);
	assert_non_null(group);
	assert_ptr_equal(obs_scene_get_source(group), source);
	for (unsigned repeat = 0; repeat < 3; repeat++) {
		assert_ptr_equal(obs_group_from_source(source), group);
		assert_null(obs_scene_from_source(source));
		assert_false(obs_weak_source_expired(weak));
	}
	assert_borrowed_release(source, weak);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test(non_scene_sources),
		cmocka_unit_test(scene_source),
		cmocka_unit_test(group_source),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
