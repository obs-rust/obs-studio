#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

/* Characterizes obs_sceneitem_get_pos(), obs_sceneitem_get_bounds() and
 * obs_sceneitem_get_info2() against the actual libraries. Every relative value
 * below is exactly representable, so no float contraction can change it. */

static const uint32_t edges[] = {0,          0x80000000, 0x3f800000, 0xbf800000, 0x7f800000,
				 0xff800000, 0x7fc01234, 0xffc05678, 1,          0x7f7fffff};
#define EDGE_COUNT (sizeof(edges) / sizeof(*edges))

static void set_bits(struct vec2 *v, uint32_t x, uint32_t y)
{
	memcpy(&v->x, &x, sizeof(x));
	memcpy(&v->y, &y, sizeof(y));
}

static void null_items(void **state)
{
	(void)state;
	struct vec2 out;
	const uint32_t sentinel[2] = {0x7fc01234, 0x80000000};
	memcpy(&out, sentinel, sizeof(out));
	obs_sceneitem_get_pos(NULL, &out);
	assert_memory_equal(&out, sentinel, sizeof(out));
	obs_sceneitem_get_bounds(NULL, &out);
	assert_memory_equal(&out, sentinel, sizeof(out));
	obs_sceneitem_get_pos(NULL, NULL);
	obs_sceneitem_get_bounds(NULL, NULL);

	struct obs_transform_info info, before;
	memset(&info, 0x5a, sizeof(info));
	memcpy(&before, &info, sizeof(info));
	obs_sceneitem_get_info2(NULL, &info);
	assert_memory_equal(&info, &before, sizeof(info));
	obs_sceneitem_get_info2(NULL, NULL);

	/* A NULL info is ignored for a live item too, and the item is not read. */
	struct obs_scene_item item;
	memset(&item, 0xa5, sizeof(item));
	item.parent = NULL;
	unsigned char item_before[sizeof(item)];
	memcpy(item_before, &item, sizeof(item));
	obs_sceneitem_get_info2(&item, NULL);
	assert_memory_equal(&item, item_before, sizeof(item));
}

/* The fields obs_sceneitem_get_info2() writes, written the same way, so the
 * padding of the expected and the actual structure stays the sentinel. */
static void expected_info(struct obs_transform_info *info, const struct obs_scene_item *item, const struct vec2 *pos,
			  const struct vec2 *scale, const struct vec2 *bounds)
{
	info->pos = *pos;
	info->scale = *scale;
	info->bounds = *bounds;
	info->rot = item->rot;
	info->alignment = item->align;
	info->bounds_type = item->bounds_type;
	info->bounds_alignment = item->bounds_align;
	info->crop_to_bounds = item->crop_to_bounds;
}

static void absolute_bits(void **state)
{
	(void)state;
	struct obs_scene scene = {.custom_size = true, .cx = 1920, .cy = 1080};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	for (unsigned parented = 0; parented < 2; parented++) {
		for (size_t i = 0; i < EDGE_COUNT; i++) {
			struct obs_scene_item item;
			memset(&item, 0xa5, sizeof(item));
			item.parent = parented ? &scene : NULL;
			item.absolute_coordinates = true;
			item.is_group = (i & 1) != 0;
			item.is_scene = (i & 2) != 0;
			item.crop_to_bounds = (i & 1) == 0;
			set_bits(&item.pos, edges[i], edges[(i + 1) % EDGE_COUNT]);
			set_bits(&item.scale, edges[(i + 2) % EDGE_COUNT], edges[(i + 3) % EDGE_COUNT]);
			set_bits(&item.bounds, edges[(i + 4) % EDGE_COUNT], edges[(i + 5) % EDGE_COUNT]);
			memcpy(&item.rot, &edges[(i + 6) % EDGE_COUNT], sizeof(item.rot));
			item.align = (uint32_t)i * 0x01010101u;
			item.bounds_type = (enum obs_bounds_type)(i + 100);
			item.bounds_align = ~(uint32_t)i;
			unsigned char before[sizeof(item)];
			memcpy(before, &item, sizeof(item));

			struct vec2 out;
			obs_sceneitem_get_pos(&item, &out);
			assert_memory_equal(&out, &item.pos, sizeof(out));
			obs_sceneitem_get_bounds(&item, &out);
			assert_memory_equal(&out, &item.bounds, sizeof(out));

			struct obs_transform_info info, expected;
			memset(&info, 0x5a, sizeof(info));
			memcpy(&expected, &info, sizeof(info));
			expected_info(&expected, &item, &item.pos, &item.scale, &item.bounds);
			obs_sceneitem_get_info2(&item, &info);
			assert_memory_equal(&info, &expected, sizeof(info));
			assert_memory_equal(&item, before, sizeof(item));
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

struct relative_case {
	bool is_group;
	bool is_scene;
	float pos[2];
	float scale[2];
	float bounds[2];
};

/* 1920x1080 parent, scale_ref.y 540, pos (2^-10, 2^-11), bounds (0.5, 2^-10),
 * scale (-2, 3). A plain item or nested scene is nudged to the next half
 * pixel; a group is not. Only a plain item rescales by canvas_y/scale_ref.y. */
static const struct relative_case relative_cases[] = {
	{false, false, {960.5f, 540.5f}, {-4.0f, 6.0f}, {270.0f, 0.5f}},
	{true, false, {960.52734375f, 540.263671875f}, {-2.0f, 3.0f}, {270.0f, 0.52734375f}},
	{false, true, {960.5f, 540.5f}, {-2.0f, 3.0f}, {270.0f, 0.5f}},
};

static void relative_conversion(void **state)
{
	(void)state;
	struct obs_scene scene = {.custom_size = true, .cx = 1920, .cy = 1080};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	for (size_t i = 0; i < sizeof(relative_cases) / sizeof(*relative_cases); i++) {
		const struct relative_case *c = &relative_cases[i];
		struct obs_scene_item item = {.parent = &scene};
		item.is_group = c->is_group;
		item.is_scene = c->is_scene;
		item.pos.x = 0.0009765625f;
		item.pos.y = 0.00048828125f;
		item.scale.x = -2.0f;
		item.scale.y = 3.0f;
		item.scale_ref.y = 540.0f;
		item.bounds.x = 0.5f;
		item.bounds.y = 0.0009765625f;
		item.rot = -45.5f;
		item.align = 5;
		item.bounds_type = OBS_BOUNDS_SCALE_INNER;
		item.bounds_align = 10;
		item.crop_to_bounds = true;
		unsigned char before[sizeof(item)];
		memcpy(before, &item, sizeof(item));

		struct vec2 out;
		obs_sceneitem_get_pos(&item, &out);
		assert_float_equal(out.x, c->pos[0], 0.0f);
		assert_float_equal(out.y, c->pos[1], 0.0f);
		obs_sceneitem_get_bounds(&item, &out);
		assert_float_equal(out.x, c->bounds[0], 0.0f);
		assert_float_equal(out.y, c->bounds[1], 0.0f);

		struct obs_transform_info info;
		memset(&info, 0x5a, sizeof(info));
		obs_sceneitem_get_info2(&item, &info);
		assert_float_equal(info.pos.x, c->pos[0], 0.0f);
		assert_float_equal(info.pos.y, c->pos[1], 0.0f);
		assert_float_equal(info.scale.x, c->scale[0], 0.0f);
		assert_float_equal(info.scale.y, c->scale[1], 0.0f);
		assert_float_equal(info.bounds.x, c->bounds[0], 0.0f);
		assert_float_equal(info.bounds.y, c->bounds[1], 0.0f);
		assert_float_equal(info.rot, -45.5f, 0.0f);
		assert_int_equal(info.alignment, 5);
		assert_int_equal(info.bounds_type, OBS_BOUNDS_SCALE_INNER);
		assert_int_equal(info.bounds_alignment, 10);
		assert_true(info.crop_to_bounds);
		assert_memory_equal(&item, before, sizeof(item));

		/* An output that is the stored field itself gets the same value. */
		obs_sceneitem_get_pos(&item, &item.pos);
		assert_float_equal(item.pos.x, c->pos[0], 0.0f);
		assert_float_equal(item.pos.y, c->pos[1], 0.0f);
		obs_sceneitem_get_bounds(&item, &item.bounds);
		assert_float_equal(item.bounds.x, c->bounds[0], 0.0f);
		assert_float_equal(item.bounds.y, c->bounds[1], 0.0f);
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

enum getter { GET_POS, GET_BOUNDS, GET_INFO2 };

struct field_read {
	enum getter getter;
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	struct vec2 out;
	struct obs_transform_info info;
};

static void *read_fields(void *data)
{
	struct field_read *read = data;
	os_event_signal(read->entered);
	if (read->getter == GET_POS)
		obs_sceneitem_get_pos(read->item, &read->out);
	else if (read->getter == GET_BOUNDS)
		obs_sceneitem_get_bounds(read->item, &read->out);
	else
		obs_sceneitem_get_info2(read->item, &read->info);
	os_event_signal(read->finished);
	return NULL;
}

static void parent_video_mutex(void **state)
{
	(void)state;
	for (int getter = GET_POS; getter <= GET_INFO2; getter++) {
		struct obs_scene scene = {0};
		struct obs_scene_item item = {.parent = &scene, .absolute_coordinates = true};
		struct field_read read = {.getter = (enum getter)getter, .item = &item};
		assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
		assert_int_equal(os_event_init(&read.entered, OS_EVENT_TYPE_AUTO), 0);
		assert_int_equal(os_event_init(&read.finished, OS_EVENT_TYPE_AUTO), 0);
		assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
		pthread_t thread;
		assert_int_equal(pthread_create(&thread, NULL, read_fields, &read), 0);
		assert_int_equal(os_event_timedwait(read.entered, 10000), 0);
		int blocked = os_event_timedwait(read.finished, 20);
		item.pos.x = -3.0f;
		item.pos.y = 5.0f;
		item.bounds.x = 7.0f;
		item.bounds.y = -9.0f;
		item.rot = 11.0f;
		item.crop_to_bounds = true;
		assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
		assert_int_equal(pthread_join(thread, NULL), 0);
		assert_int_equal(blocked, ETIMEDOUT);
		if (getter == GET_POS) {
			assert_memory_equal(&read.out, &item.pos, sizeof(read.out));
		} else if (getter == GET_BOUNDS) {
			assert_memory_equal(&read.out, &item.bounds, sizeof(read.out));
		} else {
			assert_memory_equal(&read.info.pos, &item.pos, sizeof(item.pos));
			assert_memory_equal(&read.info.bounds, &item.bounds, sizeof(item.bounds));
			assert_float_equal(read.info.rot, 11.0f, 0.0f);
			assert_true(read.info.crop_to_bounds);
		}
		os_event_destroy(read.finished);
		os_event_destroy(read.entered);
		assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_items),
		cmocka_unit_test(absolute_bits),
		cmocka_unit_test(relative_conversion),
		cmocka_unit_test(parent_video_mutex),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
