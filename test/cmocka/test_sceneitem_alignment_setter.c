/* Characterize the live library and the exact extracted body separately.
 * Stack fixtures need no OBS startup, graphics context, or worker threads. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <errno.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

/* Dependency probes make capture/store/unlock/update order observable without
 * replacing any library symbol. The public entry point below still uses libobs. */
#define obs_sceneitem_set_alignment alignment_body
#define obs_sceneitem_fields_lock alignment_lock
#define obs_sceneitem_fields_unlock alignment_unlock
#define obs_sceneitem_bounds_update_transform alignment_update
#include "libobs/obs-scene-item-alignment-setter.c.h"
#undef obs_sceneitem_bounds_update_transform
#undef obs_sceneitem_fields_unlock
#undef obs_sceneitem_fields_lock
#undef obs_sceneitem_set_alignment

static struct obs_scene_item *expected_item;
static obs_scene_t *captured_parent;
static obs_scene_t *replacement_parent;
static uint32_t old_alignment;
static uint32_t new_alignment;
static unsigned phase;
static unsigned diagnostics;
static unsigned width_calls;
static unsigned height_calls;
static unsigned transform_signals;

obs_scene_t *alignment_lock(const obs_sceneitem_t *item)
{
	assert_ptr_equal(item, expected_item);
	assert_int_equal(phase++, 0);
	assert_int_equal(item->align, old_alignment);
	obs_scene_t *parent = item->parent;
	assert_ptr_equal(parent, captured_parent);
	if (parent)
		assert_int_equal(pthread_mutex_lock(&parent->video_mutex), 0);
	/* Model a changed parent after capture: unlock must use the returned scene. */
	expected_item->parent = replacement_parent;
	return parent;
}

void alignment_unlock(obs_scene_t *scene)
{
	assert_int_equal(phase++, 1);
	assert_ptr_equal(scene, captured_parent);
	assert_int_equal(expected_item->align, new_alignment);
	if (scene) {
		assert_int_equal(pthread_mutex_trylock(&scene->video_mutex), EBUSY);
		assert_int_equal(pthread_mutex_unlock(&scene->video_mutex), 0);
	}
}

void alignment_update(obs_sceneitem_t *item)
{
	assert_int_equal(phase++, 2);
	assert_ptr_equal(item, expected_item);
	assert_int_equal(item->align, new_alignment);
	if (captured_parent) {
		assert_int_equal(pthread_mutex_trylock(&captured_parent->video_mutex), 0);
		assert_int_equal(pthread_mutex_unlock(&captured_parent->video_mutex), 0);
	}
}

static void count_diagnostic(int level, const char *format, va_list args, void *data)
{
	(void)level;
	(void)format;
	(void)args;
	(void)data;
	diagnostics++;
}

static void null_is_silent(void **state)
{
	(void)state;
	phase = diagnostics = 0;
	base_set_log_handler(count_diagnostic, NULL);
	obs_sceneitem_set_alignment(NULL, UINT32_MAX);
	alignment_body(NULL, UINT32_MAX);
	base_set_log_handler(NULL, NULL);
	assert_int_equal(diagnostics, 0);
	assert_int_equal(phase, 0);
}

static void captured_parent_and_order(void **state)
{
	(void)state;
	struct obs_scene scene = {0}, other = {0};
	/* Nonrecursive: an update before unlock cannot pass the trylock probe. */
	assert_int_equal(pthread_mutex_init(&scene.video_mutex, NULL), 0);
	const uint32_t bits[] = {0, OBS_ALIGN_LEFT | OBS_ALIGN_TOP, OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM,
				 UINT32_C(0x80000000), UINT32_MAX};
	for (unsigned parented = 0; parented < 2; parented++) {
		for (size_t i = 0; i < sizeof(bits) / sizeof(bits[0]); i++) {
			for (unsigned unchanged = 0; unchanged < 2; unchanged++) {
				struct obs_scene_item item = {0};
				captured_parent = parented ? &scene : NULL;
				replacement_parent = parented ? &other : &scene;
				item.parent = captured_parent;
				old_alignment = unchanged ? bits[i] : ~bits[i];
				item.align = old_alignment;
				new_alignment = bits[i];
				expected_item = &item;
				phase = 0;
				alignment_body(&item, new_alignment);
				assert_int_equal(phase, 3);
				assert_ptr_equal(item.parent, replacement_parent);
				assert_int_equal(item.align, new_alignment);
			}
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

static uint32_t source_width(void *data)
{
	assert_non_null(data);
	width_calls++;
	return 16;
}

static uint32_t source_height(void *data)
{
	assert_non_null(data);
	height_calls++;
	return 8;
}

static void transformed(void *data, calldata_t *params)
{
	assert_ptr_equal(calldata_ptr(params, "item"), expected_item);
	assert_ptr_equal(calldata_ptr(params, "scene"), data);
	assert_int_equal(expected_item->align, new_alignment);
	transform_signals++;
}

static void live_update_paths(void **state)
{
	(void)state;
	struct obs_source source = {0};
	struct obs_scene scene = {.source = &source};
	source.context.data = &source;
	source.info.type = OBS_SOURCE_TYPE_INPUT;
	source.info.get_width = source_width;
	source.info.get_height = source_height;
	source.context.signals = signal_handler_create();
	assert_non_null(source.context.signals);
	assert_true(signal_handler_add(source.context.signals, "void item_transform(ptr scene, ptr item)"));
	signal_handler_connect(source.context.signals, "item_transform", transformed, &scene);
	assert_int_equal(pthread_mutex_init_recursive(&source.filter_mutex), 0);
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	const uint32_t bits[] = {0, OBS_ALIGN_LEFT | OBS_ALIGN_TOP, OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM,
				 UINT32_C(0x80000000), UINT32_MAX};
	const long defers[] = {-1, 0, 1, 3};
	for (unsigned path = 0; path < 3; path++) {
		for (size_t d = 0; d < sizeof(defers) / sizeof(defers[0]); d++) {
			for (size_t i = 0; i < sizeof(bits) / sizeof(bits[0]); i++) {
				struct obs_scene_item item = {0};
				item.parent = path == 0 ? NULL : &scene;
				scene.is_group = path == 1;
				item.source = &source;
				item.absolute_coordinates = true;
				vec2_set(&item.scale, 1.0f, 1.0f);
				item.defer_update = defers[d];
				expected_item = &item;
				new_alignment = bits[i];
				for (unsigned repeat = 0; repeat < 2; repeat++) {
					width_calls = height_calls = transform_signals = 0;
					item.update_transform = false;
					obs_sceneitem_set_alignment(&item, new_alignment);
					/* Unknown/conflicting bits and negative defer counts are
					 * characterized, not endorsed. Unchanged values still update. */
					assert_int_equal(item.align, new_alignment);
					assert_int_equal(item.defer_update, defers[d]);
					assert_int_equal(item.update_transform, path != 2);
					const unsigned immediate = path == 2 && defers[d] <= 0;
					assert_int_equal(width_calls, immediate);
					assert_int_equal(height_calls, immediate);
					assert_int_equal(transform_signals, immediate);
					if (immediate) {
						assert_int_equal(item.last_width, 16);
						assert_int_equal(item.last_height, 8);
					}
					assert_int_equal(pthread_mutex_trylock(&scene.video_mutex), 0);
					assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
				}
			}
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&source.filter_mutex), 0);
	signal_handler_destroy(source.context.signals);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_is_silent),
					   cmocka_unit_test(captured_parent_and_order),
					   cmocka_unit_test(live_update_paths)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
