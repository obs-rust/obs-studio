/* Pin the C constructor contract before extraction and a later Rust swap.
 * Uses only public APIs, against both actual libobs implementations. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>
#include <string.h>

struct creation_probe {
	unsigned count;
	obs_source_t *source;
};

static void created(void *data, calldata_t *params)
{
	struct creation_probe *probe = data;
	probe->count++;
	probe->source = calldata_ptr(params, "source");
	assert_non_null(probe->source);
	/* Notifications run synchronously after the scene has been initialized. */
	assert_non_null(obs_scene_from_source(probe->source));
	assert_string_equal(obs_source_get_id(probe->source), "scene");
}

static void scene_signal(void *data, calldata_t *params)
{
	unsigned *count = data;
	assert_non_null(calldata_ptr(params, "scene"));
	(*count)++;
}

static void exercise_constructor(bool private, const char *name)
{
	struct creation_probe global = {0}, canvas = {0};
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_non_null(main_canvas);
	signal_handler_t *global_handler = obs_get_signal_handler();
	signal_handler_t *canvas_handler = global_handler;
	signal_handler_connect(global_handler, "source_create", created, &global);
	signal_handler_connect(canvas_handler, "source_create_canvas", created, &canvas);

	obs_scene_t *scene = private ? obs_scene_create_private(name) : obs_scene_create(name);
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	assert_non_null(source);
	assert_ptr_equal(obs_scene_from_source(source), scene);
	assert_null(obs_group_from_source(source));
	assert_true(obs_source_is_scene(source));
	assert_false(obs_source_is_group(source));
	assert_string_equal(obs_source_get_id(source), "scene");
	assert_int_equal(obs_source_get_type(source), OBS_SOURCE_TYPE_SCENE);
	assert_int_equal(obs_obj_is_private(source), private);
	obs_canvas_t *actual_canvas = obs_source_get_canvas(source);
	assert_ptr_equal(actual_canvas, main_canvas);
	obs_canvas_release(actual_canvas);

	const char *actual_name = obs_source_get_name(source);
	if (private && !name) {
		/* A private NULL name stays NULL: characterized, not endorsed. */
		assert_null(actual_name);
	} else if (name && *name) {
		assert_string_equal(actual_name, name);
	} else {
		assert_non_null(actual_name);
		assert_int_equal(strncmp(actual_name, "__unnamed", 9), 0);
		assert_true(strlen(actual_name) >= 13);
	}
	if (actual_name) {
		obs_source_t *found = obs_get_source_by_name(actual_name);
		if (private)
			assert_null(found);
		else
			assert_ptr_equal(found, source);
		obs_source_release(found);
	}
	assert_int_equal(global.count, private ? 0 : 1);
	assert_int_equal(canvas.count, private ? 0 : 1);
	if (!private) {
		assert_ptr_equal(global.source, source);
		assert_ptr_equal(canvas.source, source);
	}
	signal_handler_disconnect(global_handler, "source_create", created, &global);
	signal_handler_disconnect(canvas_handler, "source_create_canvas", created, &canvas);

	obs_data_t *settings = obs_source_get_settings(source);
	assert_non_null(settings);
	obs_data_item_t *first = obs_data_first(settings);
	assert_null(first);
	/* Repeated getters retain the same new settings object. */
	obs_data_t *again = obs_source_get_settings(source);
	assert_ptr_equal(again, settings);
	obs_data_release(again);
	obs_data_release(settings);

	/* Every scene-specific signal is registered by construction. Calling
	 * through the handler proves presence without private-layout access. */
	const char *signals[] = {"item_add",    "item_remove",   "reorder",        "refresh",    "item_visible",
				 "item_select", "item_deselect", "item_transform", "item_locked"};
	signal_handler_t *handler = obs_source_get_signal_handler(source);
	assert_non_null(handler);
	unsigned calls = 0;
	calldata_t params;
	calldata_init(&params);
	calldata_set_ptr(&params, "scene", scene);
	for (size_t i = 0; i < sizeof(signals) / sizeof(signals[0]); i++) {
		signal_handler_connect(handler, signals[i], scene_signal, &calls);
		signal_handler_signal(handler, signals[i], &params);
		assert_int_equal(calls, i + 1);
		signal_handler_disconnect(handler, signals[i], scene_signal, &calls);
	}
	calldata_free(&params);

	/* The caller receives one strong reference; public scenes also have a
	 * canvas-owned reference. A scene get_ref retains the same object. */
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_non_null(weak);
	obs_scene_t *retained = obs_scene_get_ref(scene);
	assert_ptr_equal(retained, scene);
	obs_scene_release(scene);
	assert_false(obs_weak_source_expired(weak));
	obs_source_t *alive = obs_weak_source_get_source(weak);
	assert_ptr_equal(alive, source);
	obs_source_release(alive);
	obs_scene_release(retained);
	while (obs_wait_for_destroy_queue())
		;
	if (!private) {
		/* Public scenes stay alive in the main canvas after the caller's
		 * last release. Remove that ownership through the public API. */
		assert_false(obs_weak_source_expired(weak));
		alive = obs_weak_source_get_source(weak);
		assert_ptr_equal(alive, source);
		obs_canvas_scene_remove(obs_scene_from_source(alive));
		obs_source_release(alive);
		while (obs_wait_for_destroy_queue())
			;
	}
	assert_true(obs_weak_source_expired(weak));
	assert_null(obs_weak_source_get_source(weak));
	obs_weak_source_release(weak);
	obs_canvas_release(main_canvas);
}

static void public_names(void **state)
{
	(void)state;
	exercise_constructor(false, "constructor public");
	exercise_constructor(false, NULL);
	exercise_constructor(false, "");
}

static void private_names(void **state)
{
	(void)state;
	exercise_constructor(true, "constructor private");
	exercise_constructor(true, NULL);
	exercise_constructor(true, "");
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(public_names),
		cmocka_unit_test(private_names),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
