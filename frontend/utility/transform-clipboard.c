#include "transform-clipboard.h"

#include <math.h>

#include <graphics/matrix4.h>

/* How much the group scales its items along x and y on the canvas: the
 * lengths of the axes of its draw transform, which include its scale or
 * bounding box. */
static void group_scale(const struct matrix4 *draw, struct vec2 *scale)
{
	scale->x = sqrtf(draw->x.x * draw->x.x + draw->x.y * draw->x.y);
	scale->y = sqrtf(draw->y.x * draw->y.x + draw->y.y * draw->y.y);
}

/* Group coordinates to canvas coordinates, the way ungrouping an item does
 * (get_ungrouped_transform() in obs-scene.c), with the bounding box scaled
 * too. A group that is both rotated and scaled unevenly would skew its items,
 * which a transform cannot express; like ungrouping, this keeps the item's
 * position, rotation and size along its own axes. */
static void to_canvas(obs_sceneitem_t *group, struct obs_transform_info *info)
{
	struct matrix4 draw;
	struct vec3 pos;
	struct vec2 scale;

	obs_sceneitem_get_draw_transform(group, &draw);
	group_scale(&draw, &scale);

	vec3_set(&pos, info->pos.x, info->pos.y, 0.0f);
	vec3_transform(&pos, &pos, &draw);
	vec2_set(&info->pos, pos.x, pos.y);

	vec2_mul(&info->scale, &info->scale, &scale);
	vec2_mul(&info->bounds, &info->bounds, &scale);
	info->rot += obs_sceneitem_get_rot(group);
}

/* Canvas coordinates to group coordinates, the inverse of to_canvas() and of
 * what adding an item to a group does (apply_group_transform()). */
static void from_canvas(obs_sceneitem_t *group, struct obs_transform_info *info)
{
	struct matrix4 draw, inv;
	struct vec3 pos;
	struct vec2 scale;

	obs_sceneitem_get_draw_transform(group, &draw);
	group_scale(&draw, &scale);
	if (scale.x == 0.0f || scale.y == 0.0f || !matrix4_inv(&inv, &draw))
		return;

	vec3_set(&pos, info->pos.x, info->pos.y, 0.0f);
	vec3_transform(&pos, &pos, &inv);
	vec2_set(&info->pos, pos.x, pos.y);

	vec2_div(&info->scale, &info->scale, &scale);
	vec2_div(&info->bounds, &info->bounds, &scale);
	info->rot -= obs_sceneitem_get_rot(group);
}

void transform_clipboard_copy(obs_scene_t *scene, obs_sceneitem_t *item, struct obs_transform_info *info,
			      struct obs_sceneitem_crop *crop)
{
	obs_sceneitem_get_info2(item, info);
	obs_sceneitem_get_crop(item, crop);

	obs_sceneitem_t *group = obs_sceneitem_get_group(scene, item);
	if (group)
		to_canvas(group, info);
}

struct paste_data {
	const struct obs_transform_info *info;
	const struct obs_sceneitem_crop *crop;
	obs_sceneitem_t *group;
};

static bool paste_item(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	struct paste_data *data = param;
	UNUSED_PARAMETER(scene);

	if (obs_sceneitem_selected(item) && !obs_sceneitem_locked(item)) {
		struct obs_transform_info info = *data->info;
		if (data->group)
			from_canvas(data->group, &info);

		obs_sceneitem_defer_update_begin(item);
		obs_sceneitem_set_info2(item, &info);
		obs_sceneitem_set_crop(item, data->crop);
		obs_sceneitem_defer_update_end(item);
	}

	/* The group has been pasted (and its transform updated) first, so its
	 * items are placed through its new transform. */
	if (obs_sceneitem_is_group(item)) {
		struct paste_data group_data = {data->info, data->crop, item};
		obs_sceneitem_group_enum_items(item, paste_item, &group_data);
	}
	return true;
}

void transform_clipboard_paste(obs_scene_t *scene, const struct obs_transform_info *info,
			       const struct obs_sceneitem_crop *crop)
{
	struct paste_data data = {info, crop, NULL};
	obs_scene_enum_items(scene, paste_item, &data);
}
