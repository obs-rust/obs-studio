#include "libobs/build/libobs.pch.h"

#define video_lock(scene) pthread_mutex_lock(&scene->video_mutex)
#define video_unlock(scene) pthread_mutex_unlock(&scene->video_mutex)

/* The transform fields of an item: the render path reads them under the
 * parent's video lock, so a setter or getter on another thread takes it too.
 * Hold it only for the fields: no signal, and no call into a plugin (#3569). */
static inline obs_scene_t *item_fields_lock(const obs_sceneitem_t *item)
{
	obs_scene_t *scene = item->parent;
	if (scene)
		video_lock(scene);
	return scene;
}

static inline void item_fields_unlock(obs_scene_t *scene)
{
	if (scene)
		video_unlock(scene);
}

/* Private conversion boundaries of obs-scene.c.h: the scene/canvas dimension
 * path stays there. */
void obs_sceneitem_pos_canvas_read(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_size_canvas_read(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_scale_canvas_read(struct vec2 *dst, const obs_sceneitem_t *item);
#define pos_to_absolute obs_sceneitem_pos_canvas_read
#define size_to_absolute obs_sceneitem_size_canvas_read
#define item_canvas_scale obs_sceneitem_scale_canvas_read

void obs_sceneitem_get_pos(const obs_sceneitem_t *item, struct vec2 *pos)
{
	if (!item)
		return;

	obs_scene_t *scene = item_fields_lock(item);
	if (!item->absolute_coordinates)
		pos_to_absolute(pos, &item->pos, item);
	else
		vec2_copy(pos, &item->pos);
	item_fields_unlock(scene);
}

void obs_sceneitem_get_bounds(const obs_sceneitem_t *item, struct vec2 *bounds)
{
	if (!item)
		return;

	obs_scene_t *scene = item_fields_lock(item);
	if (!item->absolute_coordinates)
		size_to_absolute(bounds, &item->bounds, item);
	else
		vec2_copy(bounds, &item->bounds);
	item_fields_unlock(scene);
}

static inline void scene_item_get_info_internal(const obs_sceneitem_t *item, struct obs_transform_info *info)
{
	if (!item->absolute_coordinates) {
		pos_to_absolute(&info->pos, &item->pos, item);
		item_canvas_scale(&info->scale, item);
		size_to_absolute(&info->bounds, &item->bounds, item);
	} else {
		info->pos = item->pos;
		info->scale = item->scale;
		info->bounds = item->bounds;
	}
	info->rot = item->rot;
	info->alignment = item->align;
	info->bounds_type = item->bounds_type;
	info->bounds_alignment = item->bounds_align;
}

void obs_sceneitem_get_info2(const obs_sceneitem_t *item, struct obs_transform_info *info)
{
	if (item && info) {
		obs_scene_t *scene = item_fields_lock(item);
		scene_item_get_info_internal(item, info);
		info->crop_to_bounds = item->crop_to_bounds;
		item_fields_unlock(scene);
	}
}

#undef item_canvas_scale
#undef size_to_absolute
#undef pos_to_absolute
#undef video_lock
#undef video_unlock
