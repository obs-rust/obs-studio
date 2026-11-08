/* Characterize the borrowed parent query against the actual linked libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

static void parent_getter(void **state)
{
	(void)state;
	assert_null(obs_sceneitem_get_scene(NULL));
	for (unsigned bits = 0; bits < 16; bits++) {
		struct obs_scene scenes[2] = {0};
		struct obs_scene_item item = {0};
		unsigned char before[sizeof(item)], scenes_before[sizeof(scenes)];
		item.is_group = (bits & 1) != 0;
		item.removed = (bits & 2) != 0;
		item.id = -17;
		item.parent = (bits & 4) ? &scenes[(bits >> 3) & 1] : NULL;
		memcpy(before, &item, sizeof(item));
		memcpy(scenes_before, scenes, sizeof(scenes));
		assert_ptr_equal(obs_sceneitem_get_scene(&item), item.parent);
		assert_ptr_equal(obs_sceneitem_get_scene(&item), item.parent);
		assert_memory_equal(before, &item, sizeof(item));
		assert_memory_equal(scenes_before, scenes, sizeof(scenes));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(parent_getter)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
