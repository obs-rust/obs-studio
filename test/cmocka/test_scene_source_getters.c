/* Characterize borrowed source getters against the actual linked libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "obs-scene.h"

static void scene_source_getters(void **state)
{
	(void)state;
	assert_null(obs_scene_get_source(NULL));
	assert_null(obs_sceneitem_get_source(NULL));
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	struct obs_source source = {0};
	assert_null(obs_scene_get_source(&scene));
	assert_null(obs_sceneitem_get_source(&item));
	scene.source = &source;
	item.source = &source;
	struct obs_scene scene_before;
	struct obs_scene_item item_before;
	memcpy(&scene_before, &scene, sizeof(scene));
	memcpy(&item_before, &item, sizeof(item));
	assert_ptr_equal(obs_scene_get_source(&scene), &source);
	assert_ptr_equal(obs_sceneitem_get_source(&item), &source);
	assert_ptr_equal(obs_scene_get_source(&scene), obs_sceneitem_get_source(&item));
	assert_memory_equal(&scene, &scene_before, sizeof(scene));
	assert_memory_equal(&item, &item_before, sizeof(item));
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(scene_source_getters)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
