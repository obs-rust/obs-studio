/* The original extracted bodies; production implementations enter by callback. */
#define obs_sceneitem_get_draw_transform oracle_obs_sceneitem_get_draw_transform
#define obs_sceneitem_get_box_transform oracle_obs_sceneitem_get_box_transform
#define obs_sceneitem_get_box_scale oracle_obs_sceneitem_get_box_scale
#include "obs-scene-item-matrix-getters.c.h"
#undef obs_sceneitem_get_draw_transform
#undef obs_sceneitem_get_box_transform
#undef obs_sceneitem_get_box_scale

typedef void (*matrix_getter)(const obs_sceneitem_t *, struct matrix4 *);
typedef void (*scale_getter)(const obs_sceneitem_t *, struct vec2 *);
unsigned oracle_scene_item_matrix_probe(const uint32_t *, unsigned, matrix_getter, matrix_getter, scale_getter);

unsigned oracle_scene_item_matrix_probe(const uint32_t *bits, unsigned flags, matrix_getter draw, matrix_getter box,
					scale_getter scale)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		return 128;
	struct obs_scene_item item;
	memset(&item, 0xa5, sizeof(item));
	item.parent = (flags & 1) ? &scene : NULL;
	memcpy(&item.draw_transform, bits, sizeof(item.draw_transform));
	memcpy(&item.box_transform, bits + 16, sizeof(item.box_transform));
	memcpy(&item.box_scale, bits + 32, sizeof(item.box_scale));
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	unsigned result = 0;
	/* Guard both sides of output; also compare padding and arbitrary NaN bits. */
	struct {
		struct matrix4 pre, value, post;
	} actual, expected;
	struct {
		struct vec2 pre, value, post;
	} av, ev;
	memset(&actual, 0x96, sizeof(actual));
	memset(&av, 0x69, sizeof(av));
	memcpy(&expected, &actual, sizeof(actual));
	memcpy(&ev, &av, sizeof(av));
	draw(NULL, &actual.value);
	box(NULL, &actual.value);
	scale(NULL, &av.value);
	draw(NULL, NULL);
	box(NULL, NULL);
	scale(NULL, NULL);
	if (memcmp(&actual, &expected, sizeof(actual)) || memcmp(&av, &ev, sizeof(av)))
		result |= 1;
	if ((flags & 2) && pthread_mutex_lock(&scene.video_mutex))
		result |= 128;
	draw(&item, &actual.value);
	oracle_obs_sceneitem_get_draw_transform(&item, &expected.value);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 2;
	box(&item, &actual.value);
	oracle_obs_sceneitem_get_box_transform(&item, &expected.value);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 4;
	scale(&item, &av.value);
	oracle_obs_sceneitem_get_box_scale(&item, &ev.value);
	if (memcmp(&av, &ev, sizeof(av)))
		result |= 8;
	if (memcmp(before, &item, sizeof(item)))
		result |= 16;
	/* Output may alias its stored field: no reference to the whole item exists. */
	draw(&item, &item.draw_transform);
	box(&item, &item.box_transform);
	scale(&item, &item.box_scale);
	if (memcmp(before, &item, sizeof(item)))
		result |= 32;
	if ((flags & 2) && pthread_mutex_unlock(&scene.video_mutex))
		result |= 128;
	if (pthread_mutex_trylock(&scene.video_mutex))
		result |= 64;
	else
		pthread_mutex_unlock(&scene.video_mutex);
	if (pthread_mutex_destroy(&scene.video_mutex))
		result |= 128;
	return result;
}
