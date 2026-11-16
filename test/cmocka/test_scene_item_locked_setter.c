#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "obs-scene.h"

struct locked_capture {
	obs_scene_t *scene;
	obs_sceneitem_t *item;
	unsigned count;
	bool value;
};

static void locked_signal(void *data, calldata_t *cd)
{
	struct locked_capture *capture = data;
	assert_ptr_equal(calldata_ptr(cd, "scene"), capture->scene);
	assert_ptr_equal(calldata_ptr(cd, "item"), capture->item);
	assert_int_equal(calldata_bool(cd, "locked"), capture->value);
	assert_int_equal(obs_sceneitem_locked(capture->item), capture->value);
	capture->count++;
}

static void locked_setter(void **state)
{
	(void)state;
	assert_false(obs_sceneitem_set_locked(NULL, false));
	assert_false(obs_sceneitem_set_locked(NULL, true));
	struct obs_source source = {0};
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	scene.source = &source;
	source.context.signals = signal_handler_create();
	assert_non_null(source.context.signals);
	assert_true(signal_handler_add(source.context.signals, "void item_locked(ptr scene, ptr item, bool locked)"));
	struct locked_capture capture = {.scene = &scene, .item = &item};
	signal_handler_connect(source.context.signals, "item_locked", locked_signal, &capture);
	for (unsigned parent = 0; parent < 2; parent++) {
		item.parent = parent ? &scene : NULL;
		for (unsigned initial = 0; initial < 2; initial++) {
			for (unsigned requested = 0; requested < 2; requested++) {
				item.locked = initial;
				capture.value = requested;
				capture.count = 0;
				unsigned char before[sizeof(item)];
				memcpy(before, &item, sizeof(item));
				bool changed = parent && initial != requested;
				assert_int_equal(obs_sceneitem_set_locked(&item, requested), changed);
				assert_int_equal(capture.count, changed ? 1 : 0);
				assert_int_equal(item.locked, changed ? requested : initial);
				item.locked = initial;
				assert_memory_equal(before, &item, sizeof(item));
			}
		}
	}
	signal_handler_destroy(source.context.signals);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(locked_setter)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
