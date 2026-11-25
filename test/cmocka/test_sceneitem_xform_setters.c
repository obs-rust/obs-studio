/* Direct characterization of the three public transform setters. Stack
 * fixtures exercise the live library without OBS startup or graphics threads. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

static unsigned diagnostics, width_calls, height_calls, signals;
static obs_sceneitem_t *expected_item;

static void log_call(int level, const char *format, va_list args, void *data)
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
	diagnostics = 0;
	base_set_log_handler(log_call, NULL);
	obs_sceneitem_set_pos(NULL, NULL);
	obs_sceneitem_set_rot(NULL, 123.0f);
	obs_sceneitem_set_scale(NULL, NULL);
	base_set_log_handler(NULL, NULL);
	assert_int_equal(diagnostics, 0);
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
	signals++;
}

static void set_value(obs_sceneitem_t *item, unsigned setter, const struct vec2 *v)
{
	if (setter == 0)
		obs_sceneitem_set_pos(item, v);
	else if (setter == 1)
		obs_sceneitem_set_rot(item, v->x);
	else
		obs_sceneitem_set_scale(item, v);
}

static void check_value(const struct obs_scene_item *item, unsigned setter, const struct vec2 *v)
{
	assert_float_equal(item->pos.x, setter == 0 ? v->x : 0.0f, 0.00001f);
	assert_float_equal(item->pos.y, setter == 0 ? v->y : 0.0f, 0.00001f);
	assert_float_equal(item->rot, setter == 1 ? v->x : 0.0f, 0.00001f);
	assert_float_equal(item->scale.x, setter == 2 ? v->x : 1.0f, 0.00001f);
	assert_float_equal(item->scale.y, setter == 2 ? v->y : 1.0f, 0.00001f);
}

static void absolute_update_paths(void **state)
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
	const long defers[] = {-1, 0, 1, 3};
	const float values[][2] = {{0.0f, 0.0f}, {-2.0f, 3.5f}, {450.0f, -720.0f}};
	for (unsigned setter = 0; setter < 3; setter++) {
		for (unsigned path = 0; path < 3; path++) {
			for (size_t d = 0; d < sizeof(defers) / sizeof(defers[0]); d++) {
				for (size_t v = 0; v < sizeof(values) / sizeof(values[0]); v++) {
					struct obs_scene_item item = {0};
					struct vec2 value;
					vec2_set(&value, values[v][0], values[v][1]);
					item.parent = path == 0 ? NULL : &scene;
					scene.is_group = path == 1;
					item.source = &source;
					item.absolute_coordinates = true;
					vec2_set(&item.scale, 1.0f, 1.0f);
					item.defer_update = defers[d];
					expected_item = &item;
					for (unsigned repeat = 0; repeat < 2; repeat++) {
						width_calls = height_calls = signals = 0;
						item.update_transform = false;
						set_value(&item, setter, &value);
						check_value(&item, setter, &value);
						/* Negative defers and unnormalized values: characterized, not endorsed. */
						/* Unchanged values still update. */
						assert_int_equal(item.defer_update, defers[d]);
						assert_int_equal(item.update_transform, path != 2);
						const unsigned immediate = path == 2 && defers[d] <= 0;
						assert_int_equal(width_calls, immediate);
						assert_int_equal(height_calls, immediate);
						assert_int_equal(signals, immediate);
						assert_int_equal(pthread_mutex_trylock(&scene.video_mutex), 0);
						assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
					}
				}
			}
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&source.filter_mutex), 0);
	signal_handler_destroy(source.context.signals);
}

static void relative_values_and_aliasing(void **state)
{
	(void)state;
	/* A custom-size parent avoids global video state while using the actual
	  * canvas conversion path. Deferred updates need no source or graphics. */
	struct obs_scene scene = {.custom_size = true, .cx = 1920, .cy = 1080};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	for (unsigned group = 0; group < 2; group++) {
		for (unsigned nested = 0; nested < 2; nested++) {
			struct obs_scene_item item = {.parent = &scene, .defer_update = 1};
			item.is_group = group;
			item.is_scene = nested;
			vec2_set(&item.scale_ref, 3840.0f, 2160.0f);
			struct vec2 pos, scale;
			vec2_set(&pos, 480.0f, 810.0f);
			vec2_set(&scale, -2.0f, 0.5f);
			obs_sceneitem_set_pos(&item, &pos);
			assert_float_equal(item.pos.x, -8.0f / 9.0f, 0.00001f);
			assert_float_equal(item.pos.y, 0.5f, 0.00001f);
			assert_float_equal(pos.x, 480.0f, 0.00001f);
			assert_float_equal(pos.y, 810.0f, 0.00001f);
			obs_sceneitem_set_scale(&item, &scale);
			const float factor = group || nested ? 1.0f : 2.0f;
			assert_float_equal(item.scale.x, -2.0f * factor, 0.00001f);
			assert_float_equal(item.scale.y, 0.5f * factor, 0.00001f);
			assert_float_equal(scale.x, -2.0f, 0.00001f);
			assert_float_equal(scale.y, 0.5f, 0.00001f);
			vec2_copy(&item.pos, &pos);
			obs_sceneitem_set_pos(&item, &item.pos);
			assert_float_equal(item.pos.x, -8.0f / 9.0f, 0.00001f);
			assert_float_equal(item.pos.y, 0.5f, 0.00001f);
			vec2_copy(&item.scale, &scale);
			obs_sceneitem_set_scale(&item, &item.scale);
			assert_float_equal(item.scale.x, -2.0f * factor, 0.00001f);
			assert_float_equal(item.scale.y, 0.5f * factor, 0.00001f);
			assert_int_equal(item.defer_update, 1);
			assert_false(item.update_transform);
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_is_silent), cmocka_unit_test(absolute_update_paths),
					   cmocka_unit_test(relative_values_and_aliasing)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
