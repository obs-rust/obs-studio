/* The crop integration executable links Rust's hidden rules and runs real OBS.
 * Its static scene archive needs the same private mutex boundary as libobs;
 * unlike pure-rule consumers, this executable may actually call the getters. */
#include "util/threading.h"
#include "obs-internal.h"
#include "obs-scene.h"
#include "obs-scene-coords.h"

obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item);
void obs_sceneitem_fields_unlock(obs_scene_t *scene);

obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item)
{
	obs_scene_t *scene = item->parent;
	if (scene)
		pthread_mutex_lock(&scene->video_mutex);
	return scene;
}

void obs_sceneitem_fields_unlock(obs_scene_t *scene)
{
	if (scene)
		pthread_mutex_unlock(&scene->video_mutex);
}

/* The canvas conversion boundaries. obs_sceneitem_get_info2() is a Rust shim
 * that calls these, so they cannot ask it. They read the scene/canvas size
 * through public API the way obs-scene.c.h's get_scene_dimensions() does and
 * convert with the hidden coordinate rules (obs-scene-coords.h). */
static void link_scene_dimensions(const obs_sceneitem_t *item, float *x, float *y)
{
	obs_scene_t *parent = item->parent;
	if (!parent || (parent->is_group && !parent->source->canvas)) {
		/* The main canvas: 0x0 before the first obs_reset_video(). */
		struct obs_video_info ovi;
		bool have = obs_get_video_info(&ovi);
		*x = have ? (float)ovi.base_width : 0.0f;
		*y = have ? (float)ovi.base_height : 0.0f;
	} else if (parent->is_group) {
		/* canvas_getwidth() and canvas_getheight(): the canvas's own size. */
		obs_canvas_t *canvas = obs_weak_canvas_get_canvas(parent->source->canvas);
		*x = canvas ? (float)canvas->ovi.base_width : 0.0f;
		*y = canvas ? (float)canvas->ovi.base_height : 0.0f;
		obs_canvas_release(canvas);
	} else {
		*x = (float)obs_source_get_width(parent->source);
		*y = (float)obs_source_get_height(parent->source);
	}
}

static bool link_nudge(const obs_sceneitem_t *item)
{
	return obs_scene_coords_nudge(item->is_group, item->parent && item->parent->is_group);
}

void obs_sceneitem_scale_canvas_read(struct vec2 *dst, const obs_sceneitem_t *item);
void obs_sceneitem_scale_canvas_read(struct vec2 *dst, const obs_sceneitem_t *item)
{
	if (obs_scene_coords_keep_scale(item->is_group, item->is_scene)) {
		vec2_copy(dst, &item->scale);
		return;
	}
	float x, y;
	link_scene_dimensions(item, &x, &y);
	obs_scene_item_canvas_scale(dst, &item->scale, y, item->scale_ref.y);
}

void obs_sceneitem_pos_canvas_read(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_pos_canvas_read(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	float x, y;
	link_scene_dimensions(item, &x, &y);
	obs_scene_pos_to_absolute(dst, v, x, y, link_nudge(item));
}

void obs_sceneitem_size_canvas_read(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_size_canvas_read(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	float x, y;
	link_scene_dimensions(item, &x, &y);
	obs_scene_size_to_absolute(dst, v, y, link_nudge(item));
}
