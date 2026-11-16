/* Characterize the exported kind predicates against each actual libobs.
 * Classification uses the identifier, not source type or context data. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include "obs-internal.h"

static void assert_kind(const obs_source_t *source, bool scene, bool group)
{
	for (unsigned repeat = 0; repeat < 3; repeat++) {
		assert_int_equal(obs_source_is_scene(source), scene);
		assert_int_equal(obs_source_is_group(source), group);
	}
}

static void null_source(void **state)
{
	(void)state;
	assert_kind(NULL, false, false);
}

static void public_scene(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create("kind public scene");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	assert_non_null(source);
	assert_string_equal(obs_source_get_id(source), "scene");
	assert_int_equal(obs_source_get_type(source), OBS_SOURCE_TYPE_SCENE);
	assert_kind(source, true, false);
	obs_scene_release(scene);
}

static void private_scene(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("kind private scene");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	assert_non_null(source);
	assert_string_equal(obs_source_get_id(source), "scene");
	assert_kind(source, true, false);
	obs_scene_release(scene);
}

static void group_source(void **state)
{
	(void)state;
	obs_source_t *source = obs_source_create_private("group", "kind group", NULL);
	assert_non_null(source);
	assert_non_null(obs_group_from_source(source));
	assert_string_equal(obs_source_get_id(source), "group");
	/* Scene and group share a type, but only one predicate matches. */
	assert_int_equal(obs_source_get_type(source), OBS_SOURCE_TYPE_SCENE);
	assert_kind(source, false, true);
	obs_source_release(source);
}

static const char *input_name(void *unused)
{
	(void)unused;
	return "source kind input";
}

static void *input_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void input_destroy(void *data)
{
	(void)data;
}

static void plain_inputs(void **state)
{
	(void)state;
	const char *ids[] = {"kind-input", "scene-extra", "group-extra", "Scene", "Group"};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
		struct obs_source_info info = {
			.id = ids[i],
			.type = OBS_SOURCE_TYPE_INPUT,
			.get_name = input_name,
			.create = input_create,
			.destroy = input_destroy,
		};
		obs_register_source(&info);
		obs_source_t *source = obs_source_create_private(ids[i], "kind input", NULL);
		assert_non_null(source);
		assert_string_equal(obs_source_get_id(source), ids[i]);
		assert_int_equal(obs_source_get_type(source), OBS_SOURCE_TYPE_INPUT);
		assert_kind(source, false, false);
		obs_source_release(source);
	}
}

static void identifier_only(void **state)
{
	(void)state;
	/* Minimal read-only records: these predicates read only info.id.
	 * No registration, allocation, ownership or destruction is simulated.
	 * Mutable arrays make string equality distinct from pointer identity. */
	char scene_id[] = "scene";
	char group_id[] = "group";
	struct obs_source source = {0};
	source.info.type = OBS_SOURCE_TYPE_INPUT;
	assert_null(source.context.data);
	source.info.id = scene_id;
	/* Matching ID with NULL context is characterized, not endorsed. */
	assert_kind(&source, true, false);
	source.info.id = group_id;
	assert_kind(&source, false, true);
	source.info.type = OBS_SOURCE_TYPE_SCENE;
	source.info.id = "plain";
	assert_kind(&source, false, false);
	source.info.id = "";
	assert_kind(&source, false, false);
	/* An embedded terminator ends the identifier, as in strcmp. */
	source.info.id = "scene\0suffix";
	assert_kind(&source, true, false);
	source.info.id = "group\0suffix";
	assert_kind(&source, false, true);
}

int main(void)
{
	assert_kind(NULL, false, false);
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),  cmocka_unit_test(public_scene), cmocka_unit_test(private_scene),
		cmocka_unit_test(group_source), cmocka_unit_test(plain_inputs), cmocka_unit_test(identifier_only),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	while (obs_wait_for_destroy_queue())
		;
	obs_shutdown();
	assert_kind(NULL, false, false);
	return result;
}
