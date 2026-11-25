#include "libobs/build/libobs.pch.h"

/* The existing scene/canvas conversion path, shared with the Rust setters. */
static inline void pos_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
static inline void item_relative_scale(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_xform_pos_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_xform_relative_scale(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);

void obs_sceneitem_xform_pos_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	pos_from_absolute(dst, v, item);
}

void obs_sceneitem_xform_relative_scale(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	item_relative_scale(dst, v, item);
}
