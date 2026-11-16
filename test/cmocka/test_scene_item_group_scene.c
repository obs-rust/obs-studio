#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "obs-scene.h"

/* The result is borrowed; neither the item nor its source/scene is mutated. */
static void item_group_scene(void **state)
{
	(void)state;
	assert_null(obs_sceneitem_group_get_scene(NULL));
	for (unsigned bits = 0; bits < 32; bits++) {
		struct obs_scene scenes[2] = {0};
		struct obs_source source = {0};
		struct obs_scene_item item = {0};
		item.is_group = (bits & 1) != 0;
		item.selected = (bits & 2) != 0;
		item.removed = (bits & 4) != 0;
		item.id = -17;
		source.context.data = (bits & 8) ? &scenes[(bits >> 4) & 1] : NULL;
		/* Non-groups must not dereference their source, including NULL. */
		item.source = (item.is_group || (bits & 2)) ? &source : NULL;
		unsigned char item_before[sizeof(item)], source_before[sizeof(source)], scenes_before[sizeof(scenes)];
		memcpy(item_before, &item, sizeof(item));
		memcpy(source_before, &source, sizeof(source));
		memcpy(scenes_before, scenes, sizeof(scenes));
		obs_scene_t *expected = item.is_group ? source.context.data : NULL;
		assert_ptr_equal(obs_sceneitem_group_get_scene(&item), expected);
		assert_ptr_equal(obs_sceneitem_group_get_scene(&item), expected);
		assert_memory_equal(item_before, &item, sizeof(item));
		assert_memory_equal(source_before, &source, sizeof(source));
		assert_memory_equal(scenes_before, scenes, sizeof(scenes));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(item_group_scene)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
