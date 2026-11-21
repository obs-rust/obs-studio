/* Characterize the linked API and verbatim extracted C before the Rust swap.
 * Stack fixtures own no resources and never enter the scene-item destroy path. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <limits.h>
#include <string.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

#define obs_sceneitem_addref characterized_sceneitem_addref
#include "libobs/obs-sceneitem-addref.c.h"
#undef obs_sceneitem_addref

static void null_is_noop(void **state)
{
	(void)state;
	obs_sceneitem_addref(NULL);
	characterized_sceneitem_addref(NULL);
}

static void increments_only_item_ref(void **state)
{
	(void)state;
	void (*const implementations[])(obs_sceneitem_t *) = {obs_sceneitem_addref, characterized_sceneitem_addref};
	/* Characterized, not endorsed: zero and negative stored counts also increment.
	 * Stay below LONG_MAX so every addition is representable on every host. */
	const long counts[] = {LONG_MIN, -3, -1, 0, 1, 42, LONG_MAX - 3};
	for (size_t impl = 0; impl < sizeof(implementations) / sizeof(implementations[0]); impl++) {
		for (size_t n = 0; n < sizeof(counts) / sizeof(counts[0]); n++) {
			struct obs_scene_item item, expected, neighbor, neighbor_before;
			memset(&item, 0, sizeof(item));
			memset(&neighbor, 0, sizeof(neighbor));
			item.ref = counts[n];
			item.removed = true;
			item.id = INT64_C(123456789);
			item.active_refs = 17;
			item.defer_update = 23;
			item.defer_group_resize = 29;
			item.visible = true;
			item.selected = true;
			item.locked = true;
			item.crop.left = -11;
			item.prev = &neighbor;
			item.next = &neighbor;
			neighbor.ref = 71;
			neighbor.next = &item;
			memcpy(&expected, &item, sizeof(item));
			memcpy(&neighbor_before, &neighbor, sizeof(neighbor));
			for (long increment = 1; increment <= 3; increment++) {
				implementations[impl](&item);
				expected.ref = counts[n] + increment;
				assert_int_equal(item.ref, expected.ref);
				assert_memory_equal(&item, &expected, sizeof(item));
				assert_memory_equal(&neighbor, &neighbor_before, sizeof(neighbor));
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_is_noop),
		cmocka_unit_test(increments_only_item_ref),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
