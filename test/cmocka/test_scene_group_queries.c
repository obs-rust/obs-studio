#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

static void group_queries(void **state)
{
	(void)state;
	assert_false(obs_sceneitem_is_group(NULL));
	assert_false(obs_scene_is_group(NULL));
	for (unsigned bits = 0; bits < 16; bits++) {
		struct obs_scene_item item = {0};
		struct obs_scene scene = {0};
		item.is_group = (bits & 1) != 0;
		scene.is_group = (bits & 2) != 0;
		item.is_scene = (bits & 4) != 0;
		item.removed = (bits & 8) != 0;
		item.id = -17;
		unsigned char item_before[sizeof(item)], scene_before[sizeof(scene)];
		memcpy(item_before, &item, sizeof(item));
		memcpy(scene_before, &scene, sizeof(scene));
		assert_int_equal(obs_sceneitem_is_group(&item), item.is_group);
		assert_int_equal(obs_scene_is_group(&scene), scene.is_group);
		assert_memory_equal(item_before, &item, sizeof(item));
		assert_memory_equal(scene_before, &scene, sizeof(scene));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(group_queries)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
