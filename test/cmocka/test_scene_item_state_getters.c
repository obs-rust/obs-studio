#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

/* Actual-library characterization: selection and locking are independent,
 * including detached/removed items. Neither getter mutates any item byte. */
static void state_getters(void **state)
{
	(void)state;
	assert_false(obs_sceneitem_selected(NULL));
	assert_false(obs_sceneitem_locked(NULL));
	for (unsigned bits = 0; bits < 32; bits++) {
		struct obs_scene_item item = {0};
		item.selected = (bits & 1) != 0;
		item.locked = (bits & 2) != 0;
		item.visible = (bits & 4) != 0;
		item.user_visible = (bits & 8) != 0;
		item.removed = (bits & 16) != 0;
		item.id = -17;
		unsigned char before[sizeof(item)];
		memcpy(before, &item, sizeof(item));
		assert_int_equal(obs_sceneitem_selected(&item), item.selected);
		assert_int_equal(obs_sceneitem_locked(&item), item.locked);
		assert_memory_equal(before, &item, sizeof(item));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(state_getters)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
