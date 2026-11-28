/* Original three setters, over real private headers. Scene lock and update
 * tracing are shared with the existing bounds oracle; Tier 2 covers live dispatch. */
#define obs_sceneitem_set_pos oracle_obs_sceneitem_set_pos
#define obs_sceneitem_set_rot oracle_obs_sceneitem_set_rot
#define obs_sceneitem_set_scale oracle_obs_sceneitem_set_scale
#include "libobs/build/libobs.pch.h"
#include "scene-item-xform-setters.inc"
#undef obs_sceneitem_set_scale
#undef obs_sceneitem_set_rot
#undef obs_sceneitem_set_pos

typedef void (*xform_setter)(obs_sceneitem_t *, const struct vec2 *);
unsigned oracle_scene_bounds_xform_probe(xform_setter original, xform_setter rust, unsigned which, const float *value,
					 int32_t defer, unsigned flags);
void oracle_scene_bounds_xform_record(const obs_sceneitem_t *item);
void oracle_obs_scene_pos_from_absolute(struct vec2 *dst, const struct vec2 *v, float x, float y);
void oracle_obs_scene_item_relative_scale(struct vec2 *dst, const struct vec2 *v, float y, float ref_y);
void obs_sceneitem_xform_pos_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_xform_relative_scale(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
unsigned oracle_scene_xform_probe(xform_setter rust, unsigned which, const float *value, int32_t defer, unsigned flags);

void obs_sceneitem_xform_pos_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	oracle_scene_bounds_xform_record(item);
	oracle_obs_scene_pos_from_absolute(dst, v, 1920.0f, 1080.0f);
}
void obs_sceneitem_xform_relative_scale(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	oracle_scene_bounds_xform_record(item);
	if (item->is_group || item->is_scene)
		vec2_copy(dst, v);
	else
		oracle_obs_scene_item_relative_scale(dst, v, 1080.0f, item->scale_ref.y);
}
static void xform_rotation(obs_sceneitem_t *item, const struct vec2 *v)
{
	oracle_obs_sceneitem_set_rot(item, v->x);
}
unsigned oracle_scene_xform_probe(xform_setter rust, unsigned which, const float *value, int32_t defer, unsigned flags)
{
	const xform_setter originals[] = {oracle_obs_sceneitem_set_pos, xform_rotation, oracle_obs_sceneitem_set_scale};
	return oracle_scene_bounds_xform_probe(originals[which], rust, which, value, defer, flags);
}
