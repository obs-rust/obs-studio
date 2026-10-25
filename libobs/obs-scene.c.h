/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
                          Philippe Groarke <philippe.groarke@gmail.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

const struct obs_source_info group_info;

static void resize_group(obs_sceneitem_t *group, bool scene_resize);
static void resize_scene(obs_scene_t *scene);
static void signal_parent(obs_scene_t *parent, const char *name, calldata_t *params);
static void get_ungrouped_transform(obs_sceneitem_t *group, obs_sceneitem_t *item, struct vec2 *pos, struct vec2 *scale,
				    float *rot);
static void init_hotkeys(obs_scene_t *scene, obs_sceneitem_t *item, const char *name);

typedef DARRAY(struct obs_scene_item *) obs_scene_item_ptr_array_t;

/* NOTE: For proper mutex lock order (preventing mutual cross-locks), never
 * lock the graphics mutex inside either of the scene mutexes.
 *
 * Another thing that must be done to prevent that cross-lock (and improve
 * performance), is to not create/release/update sources within the scene
 * mutexes.
 *
 * It's okay to lock the graphics mutex before locking either of the scene
 * mutexes, but not after.
 */

static const char *obs_scene_signals[] = {
	"void item_add(ptr scene, ptr item)",
	"void item_remove(ptr scene, ptr item)",
	"void reorder(ptr scene)",
	"void refresh(ptr scene)",
	"void item_visible(ptr scene, ptr item, bool visible)",
	"void item_select(ptr scene, ptr item)",
	"void item_deselect(ptr scene, ptr item)",
	"void item_transform(ptr scene, ptr item)",
	"void item_locked(ptr scene, ptr item, bool locked)",
	NULL,
};

static inline void signal_item_remove(struct obs_scene *parent, struct obs_scene_item *item)
{
	struct calldata params;
	uint8_t stack[128];

	calldata_init_fixed(&params, stack, sizeof(stack));
	calldata_set_ptr(&params, "item", item);

	signal_parent(parent, "item_remove", &params);
}

static const char *scene_getname(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "Scene";
}

static const char *group_getname(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "Group";
}

static void *scene_create(obs_data_t *settings, struct obs_source *source)
{
	struct obs_scene *scene = bzalloc(sizeof(struct obs_scene));
	scene->source = source;

	if (strcmp(source->info.id, group_info.id) == 0) {
		scene->is_group = true;
		scene->custom_size = true;
		scene->cx = 0;
		scene->cy = 0;
	}

	signal_handler_add_array(obs_source_get_signal_handler(source), obs_scene_signals);

	if (pthread_mutex_init_recursive(&scene->audio_mutex) != 0) {
		blog(LOG_ERROR, "scene_create: Couldn't initialize audio "
				"mutex");
		goto fail;
	}
	if (pthread_mutex_init_recursive(&scene->video_mutex) != 0) {
		blog(LOG_ERROR, "scene_create: Couldn't initialize video "
				"mutex");
		goto fail;
	}

	scene->absolute_coordinates = obs_data_get_bool(obs->data.private_data, "AbsoluteCoordinates");

	UNUSED_PARAMETER(settings);
	return scene;

fail:
	bfree(scene);
	return NULL;
}

#define audio_lock(scene) pthread_mutex_lock(&scene->audio_mutex)
#define video_lock(scene) pthread_mutex_lock(&scene->video_mutex)
#define audio_unlock(scene) pthread_mutex_unlock(&scene->audio_mutex)
#define video_unlock(scene) pthread_mutex_unlock(&scene->video_mutex)

static inline void full_lock(struct obs_scene *scene)
{
	video_lock(scene);
	audio_lock(scene);
}

static inline void full_unlock(struct obs_scene *scene)
{
	audio_unlock(scene);
	video_unlock(scene);
}

static void obs_sceneitem_remove_internal(obs_sceneitem_t *item);

static void remove_all_items(struct obs_scene *scene)
{
	struct obs_scene_item *item;
	obs_scene_item_ptr_array_t items;

	da_init(items);

	full_lock(scene);

	item = scene->first_item;

	while (item) {
		struct obs_scene_item *del_item = item;
		item = item->next;

		obs_sceneitem_remove_internal(del_item);
		da_push_back(items, &del_item);
	}

	full_unlock(scene);

	for (size_t i = 0; i < items.num; i++)
		obs_sceneitem_release(items.array[i]);
	da_free(items);
}

static void scene_destroy(void *data)
{
	struct obs_scene *scene = data;

	remove_all_items(scene);

	pthread_mutex_destroy(&scene->video_mutex);
	pthread_mutex_destroy(&scene->audio_mutex);
	bfree(scene);
}

static inline bool scene_transition_active(obs_source_t *transition)
{
	return transition && (transition->transitioning_audio || transition->transitioning_video);
}

static void scene_enum_sources(void *data, obs_source_enum_proc_t enum_callback, void *param, bool active)
{
	struct obs_scene *scene = data;
	struct obs_scene_item *item;
	struct obs_scene_item *next;

	full_lock(scene);
	item = scene->first_item;

	while (item) {
		next = item->next;

		obs_sceneitem_addref(item);
		if (active) {
			/* A running item transition is an active child of the
			 * scene (obs_sceneitem_do_transition), whatever the
			 * item's visibility: hiding a group starts the hide
			 * transitions of children that stay visible. */
			if (scene_transition_active(item->show_transition))
				enum_callback(scene->source, item->show_transition, param);
			else if (scene_transition_active(item->hide_transition))
				enum_callback(scene->source, item->hide_transition, param);
			if (os_atomic_load_long(&item->active_refs) > 0)
				enum_callback(scene->source, item->source, param);
		} else {
			if (item->show_transition)
				enum_callback(scene->source, item->show_transition, param);
			if (item->hide_transition)
				enum_callback(scene->source, item->hide_transition, param);
			enum_callback(scene->source, item->source, param);
		}

		obs_sceneitem_release(item);

		item = next;
	}

	full_unlock(scene);
}

static void scene_enum_active_sources(void *data, obs_source_enum_proc_t enum_callback, void *param)
{
	scene_enum_sources(data, enum_callback, param, true);
}

static void scene_enum_all_sources(void *data, obs_source_enum_proc_t enum_callback, void *param)
{
	scene_enum_sources(data, enum_callback, param, false);
}

static inline void detach_sceneitem(struct obs_scene_item *item)
{
	if (item->prev)
		item->prev->next = item->next;
	else
		item->parent->first_item = item->next;

	if (item->next)
		item->next->prev = item->prev;

	item->parent = NULL;
}

static inline void attach_sceneitem(struct obs_scene *parent, struct obs_scene_item *item, struct obs_scene_item *prev)
{
	item->prev = prev;
	item->parent = parent;

	if (prev) {
		item->next = prev->next;
		if (prev->next)
			prev->next->prev = item;
		prev->next = item;
	} else {
		item->next = parent->first_item;
		if (parent->first_item)
			parent->first_item->prev = item;
		parent->first_item = item;
	}
}

static uint32_t scene_getwidth(void *data);
static uint32_t scene_getheight(void *data);
static uint32_t canvas_getwidth(obs_weak_canvas_t *weak);
static uint32_t canvas_getheight(obs_weak_canvas_t *weak);

static inline void get_scene_dimensions(const obs_sceneitem_t *item, float *x, float *y)
{
	obs_scene_t *parent = item->parent;
	if (!parent || (parent->is_group && !parent->source->canvas)) {
		/* The main canvas has no mix until the first obs_reset_video()
		 * (#2253): 0x0 then, as scene_getwidth() and scene_getheight()
		 * answer for a scene. */
		const obs_canvas_t *canvas = obs->data.main_canvas;
		const struct obs_core_video_mix *mix = canvas ? canvas->mix : NULL;
		*x = mix ? (float)mix->ovi.base_width : 0.0f;
		*y = mix ? (float)mix->ovi.base_height : 0.0f;
	} else if (parent->is_group) {
		*x = (float)canvas_getwidth(parent->source->canvas);
		*y = (float)canvas_getheight(parent->source->canvas);
	} else {
		*x = (float)scene_getwidth(parent);
		*y = (float)scene_getheight(parent);
	}
}

/* The conversion itself is obs-scene-coords.c.h. The scene still reads the
 * canvas and still decides nudge and keep-scale. */

static inline void pos_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	float x, y;
	get_scene_dimensions(item, &x, &y);
	obs_scene_pos_from_absolute(dst, v, x, y);
}

static inline void pos_to_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	float x, y;
	get_scene_dimensions(item, &x, &y);
	obs_scene_pos_to_absolute(dst, v, x, y,
				  obs_scene_coords_nudge(item->is_group, item->parent && item->parent->is_group));
}

static inline void size_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	float x, y;
	get_scene_dimensions(item, &x, &y);
	obs_scene_size_from_absolute(dst, v, y);
}

static inline void size_to_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	float x, y;
	get_scene_dimensions(item, &x, &y);
	obs_scene_size_to_absolute(dst, v, y,
				   obs_scene_coords_nudge(item->is_group, item->parent && item->parent->is_group));
}

/* Return item's scale value scaled from original to current canvas size. */
static inline void item_canvas_scale(struct vec2 *dst, const obs_sceneitem_t *item)
{
	/* Groups will themselves resize so their items do not need to be
	 * rescaled manually. Nested scenes will use the updated canvas
	 * resolution, so they also don't need manual adjustment. */
	if (obs_scene_coords_keep_scale(item->is_group, item->is_scene)) {
		vec2_copy(dst, &item->scale);
		return;
	}

	float x, y;
	get_scene_dimensions(item, &x, &y);
	obs_scene_item_canvas_scale(dst, &item->scale, y, item->scale_ref.y);
}

/* Return scale value scaled to original canvas size. */
static inline void item_relative_scale(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	if (obs_scene_coords_keep_scale(item->is_group, item->is_scene)) {
		vec2_copy(dst, v);
		return;
	}

	float x, y;
	get_scene_dimensions(item, &x, &y);
	obs_scene_item_relative_scale(dst, v, y, item->scale_ref.y);
}

/* A bounds type outside the enum (a hand-edited or foreign scene
 * collection, undo data, a plugin) has no bounding box: OBS_BOUNDS_NONE.
 * Takes the int64 a load reads, before any cast truncates it, and an enum
 * value from a setter (an unsigned or a signed int, by compiler). */
static inline enum obs_bounds_type valid_bounds_type(int64_t type)
{
	if (type < OBS_BOUNDS_NONE || type > OBS_BOUNDS_MAX_ONLY)
		return OBS_BOUNDS_NONE;
	return (enum obs_bounds_type)type;
}

static void calculate_bounds_data(struct obs_scene_item *item, struct vec2 *origin, struct vec2 *scale, uint32_t *cx,
				  uint32_t *cy)
{
	struct vec2 bounds;
	if (item->absolute_coordinates)
		vec2_copy(&bounds, &item->bounds);
	else
		size_to_absolute(&bounds, &item->bounds, item);

	obs_bounds_place(item->bounds_type, &bounds, origin, scale, cx, cy, item->bounds_align, item->crop_to_bounds,
			 &item->bounds_crop);
}

static inline uint32_t calc_cx(const struct obs_scene_item *item, uint32_t width)
{
	return obs_scene_cropped_size(width, item->crop.left, item->crop.right, item->bounds_crop.left,
				      item->bounds_crop.right);
}

static inline uint32_t calc_cy(const struct obs_scene_item *item, uint32_t height)
{
	return obs_scene_cropped_size(height, item->crop.top, item->crop.bottom, item->bounds_crop.top,
				      item->bounds_crop.bottom);
}

#ifdef DEBUG_TRANSFORM
static inline void log_matrix(const struct matrix4 *mat, const char *name)
{
	blog(LOG_DEBUG,
	     "Matrix \"%s\":\n"
	     "┏ %9.4f %9.4f %9.4f %9.4f ┓\n"
	     "┃ %9.4f %9.4f %9.4f %9.4f ┃\n"
	     "┃ %9.4f %9.4f %9.4f %9.4f ┃\n"
	     "┗ %9.4f %9.4f %9.4f %9.4f ┛",
	     name, mat->x.x, mat->x.y, mat->x.z, mat->x.w, mat->y.x, mat->y.y, mat->y.z, mat->y.w, mat->z.x, mat->z.y,
	     mat->z.z, mat->z.w, mat->t.x, mat->t.y, mat->t.z, mat->t.w);
}
#endif

/* Needs the parent's video lock: the graphics thread reads the results.
 * The composition is obs-scene-item-transform.c.h, in steps; each
 * get_scene_dimensions() read and each float-to-integer cast stays here,
 * between them. */
static void compute_item_transform(struct obs_scene_item *item, uint32_t width, uint32_t height, bool update_tex)
{
	struct obs_scene_item_transform_size size;
	uint32_t cx;
	uint32_t cy;
	struct vec2 base_origin;
	struct vec2 origin;
	struct vec2 scale;
	struct vec2 position;

	/* Reset bounds crop, adjust crop on nested scenes (if any) */
	size = obs_scene_item_transform_prepare(&item->crop, &item->bounds_crop, &item->last_width, &item->last_height,
						width, height, update_tex && item->is_scene);
	cx = size.cx;
	cy = size.cy;

	width = cx;
	height = cy;

	vec2_zero(&base_origin);
	vec2_zero(&origin);

	if (!item->absolute_coordinates) {
		item_canvas_scale(&scale, item);
		pos_to_absolute(&position, &item->pos, item);
	} else {
		scale = item->scale;
		position = item->pos;
	}

	/* ----------------------- */

	if (obs_scene_item_transform_bounded(item->bounds_type)) {
		calculate_bounds_data(item, &origin, &scale, &cx, &cy);
	} else {
		cx = (uint32_t)((float)cx * scale.x);
		cy = (uint32_t)((float)cy * scale.y);
	}

	add_alignment(&origin, item->align, (int)cx, (int)cy);

	obs_scene_item_transform_matrix(&item->draw_transform, &scale, &origin, item->rot, &position);

#ifdef DEBUG_TRANSFORM
	blog(LOG_DEBUG, "Transform updated for \"%s\":", obs_source_get_name(item->source));
	log_matrix(&item->draw_transform, "draw_transform");
#endif

	item->output_scale = scale;

	/* ----------------------- */

	if (obs_scene_item_transform_bounded(item->bounds_type)) {
		if (!item->absolute_coordinates)
			size_to_absolute(&scale, &item->bounds, item);
		else
			vec2_copy(&scale, &item->bounds);
	} else {
		obs_scene_item_transform_box_scale(&scale, width, height);
	}

	item->box_scale = scale;

	add_alignment(&base_origin, item->align, (int)scale.x, (int)scale.y);

	obs_scene_item_transform_matrix(&item->box_transform, &scale, &base_origin, item->rot, &position);

#ifdef DEBUG_TRANSFORM
	log_matrix(&item->draw_transform, "box_transform");
#endif
}

/* Setters call this on their own thread while the graphics thread renders,
 * so the transform is computed under the parent's video lock (recursive: the
 * render path already holds it). The source size is read before locking, as
 * get_width can reach plugin code, and item_transform is sent after
 * unlocking, as its handlers can enter graphics, which the render thread
 * holds while it takes the video lock (#2115). */
static void update_item_transform(struct obs_scene_item *item, bool update_tex)
{
	struct calldata params;
	uint8_t stack[128];

	if (os_atomic_load_long(&item->defer_update) > 0)
		return;

	uint32_t width = obs_source_get_width(item->source);
	uint32_t height = obs_source_get_height(item->source);

	obs_scene_t *scene = item->parent;
	if (scene)
		video_lock(scene);
	compute_item_transform(item, width, height, update_tex);
	if (scene)
		video_unlock(scene);

	calldata_init_fixed(&params, stack, sizeof(stack));
	calldata_set_ptr(&params, "item", item);
	signal_parent(item->parent, "item_transform", &params);

	if (!update_tex)
		return;

	os_atomic_set_bool(&item->update_transform, false);
}

static inline bool source_size_changed(struct obs_scene_item *item)
{
	uint32_t width = obs_source_get_width(item->source);
	uint32_t height = obs_source_get_height(item->source);

	return item->last_width != width || item->last_height != height;
}

/* obs-scene-render-plan.h mirrors these values, so it needs no core header. */
_Static_assert(OBS_SCENE_RENDER_SCALE_DISABLE == OBS_SCALE_DISABLE, "OBS_SCALE_DISABLE");
_Static_assert(OBS_SCENE_RENDER_SCALE_POINT == OBS_SCALE_POINT, "OBS_SCALE_POINT");
_Static_assert(OBS_SCENE_RENDER_SCALE_BICUBIC == OBS_SCALE_BICUBIC, "OBS_SCALE_BICUBIC");
_Static_assert(OBS_SCENE_RENDER_SCALE_LANCZOS == OBS_SCALE_LANCZOS, "OBS_SCALE_LANCZOS");
_Static_assert(OBS_SCENE_RENDER_SCALE_AREA == OBS_SCALE_AREA, "OBS_SCALE_AREA");
_Static_assert(OBS_SCENE_RENDER_BLEND_METHOD_SRGB_OFF == OBS_BLEND_METHOD_SRGB_OFF, "OBS_BLEND_METHOD_SRGB_OFF");
_Static_assert(OBS_SCENE_RENDER_BLEND_NORMAL == OBS_BLEND_NORMAL, "OBS_BLEND_NORMAL");
_Static_assert(OBS_SCENE_RENDER_BLEND_DARKEN == OBS_BLEND_DARKEN, "OBS_BLEND_DARKEN");
_Static_assert(OBS_SCENE_RENDER_CS_SRGB == GS_CS_SRGB, "GS_CS_SRGB");
_Static_assert(OBS_SCENE_RENDER_CS_SRGB_16F == GS_CS_SRGB_16F, "GS_CS_SRGB_16F");
_Static_assert(OBS_SCENE_RENDER_CS_709_EXTENDED == GS_CS_709_EXTENDED, "GS_CS_709_EXTENDED");
_Static_assert(OBS_SCENE_RENDER_CS_709_SCRGB == GS_CS_709_SCRGB, "GS_CS_709_SCRGB");
_Static_assert(sizeof(enum obs_scale_type) == sizeof(int), "enum obs_scale_type is int-sized");
_Static_assert(sizeof(enum obs_blending_method) == sizeof(int), "enum obs_blending_method is int-sized");
_Static_assert(sizeof(enum obs_blending_type) == sizeof(int), "enum obs_blending_type is int-sized");
_Static_assert(sizeof(enum gs_color_space) == sizeof(int), "enum gs_color_space is int-sized");

/* obs-scene-item-names.h mirrors these values, so it needs no core header. */
_Static_assert(OBS_SCENE_ITEM_SCALE_DISABLE == OBS_SCALE_DISABLE, "OBS_SCALE_DISABLE");
_Static_assert(OBS_SCENE_ITEM_SCALE_POINT == OBS_SCALE_POINT, "OBS_SCALE_POINT");
_Static_assert(OBS_SCENE_ITEM_SCALE_BICUBIC == OBS_SCALE_BICUBIC, "OBS_SCALE_BICUBIC");
_Static_assert(OBS_SCENE_ITEM_SCALE_BILINEAR == OBS_SCALE_BILINEAR, "OBS_SCALE_BILINEAR");
_Static_assert(OBS_SCENE_ITEM_SCALE_LANCZOS == OBS_SCALE_LANCZOS, "OBS_SCALE_LANCZOS");
_Static_assert(OBS_SCENE_ITEM_SCALE_AREA == OBS_SCALE_AREA, "OBS_SCALE_AREA");
_Static_assert(OBS_SCENE_ITEM_BLEND_METHOD_DEFAULT == OBS_BLEND_METHOD_DEFAULT, "OBS_BLEND_METHOD_DEFAULT");
_Static_assert(OBS_SCENE_ITEM_BLEND_METHOD_SRGB_OFF == OBS_BLEND_METHOD_SRGB_OFF, "OBS_BLEND_METHOD_SRGB_OFF");
_Static_assert(OBS_SCENE_ITEM_BLEND_NORMAL == OBS_BLEND_NORMAL, "OBS_BLEND_NORMAL");
_Static_assert(OBS_SCENE_ITEM_BLEND_ADDITIVE == OBS_BLEND_ADDITIVE, "OBS_BLEND_ADDITIVE");
_Static_assert(OBS_SCENE_ITEM_BLEND_SUBTRACT == OBS_BLEND_SUBTRACT, "OBS_BLEND_SUBTRACT");
_Static_assert(OBS_SCENE_ITEM_BLEND_SCREEN == OBS_BLEND_SCREEN, "OBS_BLEND_SCREEN");
_Static_assert(OBS_SCENE_ITEM_BLEND_MULTIPLY == OBS_BLEND_MULTIPLY, "OBS_BLEND_MULTIPLY");
_Static_assert(OBS_SCENE_ITEM_BLEND_LIGHTEN == OBS_BLEND_LIGHTEN, "OBS_BLEND_LIGHTEN");
_Static_assert(OBS_SCENE_ITEM_BLEND_DARKEN == OBS_BLEND_DARKEN, "OBS_BLEND_DARKEN");

static inline bool item_texture_enabled(const struct obs_scene_item *item)
{
	const struct obs_scene_render_triggers triggers = {
		.crop_left = item->crop.left,
		.crop_top = item->crop.top,
		.crop_right = item->crop.right,
		.crop_bottom = item->crop.bottom,
		.bounds_crop_left = item->bounds_crop.left,
		.bounds_crop_top = item->bounds_crop.top,
		.bounds_crop_right = item->bounds_crop.right,
		.bounds_crop_bottom = item->bounds_crop.bottom,
		.scale_filter = item->scale_filter,
		.blend_method = item->blend_method,
		.blend_type = item->blend_type,
		.is_scene = item->source && item->source->info.type == OBS_SOURCE_TYPE_SCENE,
		.is_group = item->is_group,
	};
	return obs_scene_render_texture_enabled(&triggers);
}

/* The child render_item() draws: the show transition is asked for only when
 * the item is visible, and the hide transition only when it is not. */
static inline enum obs_scene_render_child_id item_render_child(const struct obs_scene_item *item)
{
	const bool user_visible = item->user_visible;
	const bool show_active = user_visible && scene_transition_active(item->show_transition);
	const bool hide_active = !user_visible && scene_transition_active(item->hide_transition);
	return obs_scene_render_child(user_visible, show_active, hide_active);
}

static gs_effect_t *render_effect(int effect)
{
	switch (effect) {
	case OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES:
		return obs->video.bilinear_lowres_effect;
	case OBS_SCENE_RENDER_EFFECT_BICUBIC:
		return obs->video.bicubic_effect;
	case OBS_SCENE_RENDER_EFFECT_LANCZOS:
		return obs->video.lanczos_effect;
	case OBS_SCENE_RENDER_EFFECT_AREA:
		return obs->video.area_effect;
	}
	return obs->video.default_effect;
}

static const char *render_technique_name(enum obs_scene_render_technique_id tech)
{
	switch (tech) {
	case OBS_SCENE_RENDER_TECH_DRAW:
		return "Draw";
	case OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY:
		return "DrawMultiply";
	case OBS_SCENE_RENDER_TECH_DRAW_TONEMAP:
		return "DrawTonemap";
	case OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP:
		return "DrawMultiplyTonemap";
	case OBS_SCENE_RENDER_TECH_DRAW_UPSCALE:
		return "DrawUpscale";
	case OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY:
		return "DrawUpscaleMultiply";
	case OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP:
		return "DrawUpscaleTonemap";
	case OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP:
		return "DrawUpscaleMultiplyTonemap";
	}
	return "Draw";
}

static void render_item_texture(struct obs_scene_item *item, enum gs_color_space current_space,
				enum gs_color_space source_space)
{
	gs_texture_t *tex = gs_texrender_get_texture(item->item_render);
	if (!tex) {
		return;
	}

	GS_DEBUG_MARKER_BEGIN(GS_DEBUG_COLOR_ITEM_TEXTURE, "render_item_texture");

	uint32_t cx = gs_texture_get_width(tex);
	uint32_t cy = gs_texture_get_height(tex);

	struct obs_scene_render_effect_plan plan;
	obs_scene_render_effect(item->scale_filter, item->output_scale.x, item->output_scale.y, &plan);
	gs_effect_t *effect = render_effect(plan.effect);

	if (plan.point_sampler) {
		gs_eparam_t *image = gs_effect_get_param_by_name(effect, "image");
		gs_effect_set_next_sampler(image, obs->video.point_sampler);
	}

	if (plan.base_dimension) {
		gs_eparam_t *const scale_param = gs_effect_get_param_by_name(effect, "base_dimension");
		if (scale_param) {
			struct vec2 base_res = {(float)cx, (float)cy};

			gs_effect_set_vec2(scale_param, &base_res);
		}

		gs_eparam_t *const scale_i_param = gs_effect_get_param_by_name(effect, "base_dimension_i");
		if (scale_i_param) {
			struct vec2 base_res_i = {1.0f / (float)cx, 1.0f / (float)cy};

			gs_effect_set_vec2(scale_i_param, &base_res_i);
		}
	}

	const float multiplier =
		obs_scene_render_multiplier(current_space, source_space, obs_get_video_sdr_white_level());
	const char *tech_name =
		render_technique_name(obs_scene_render_technique(plan.upscale, current_space, source_space));

	gs_eparam_t *const multiplier_param = gs_effect_get_param_by_name(effect, "multiplier");
	if (multiplier_param)
		gs_effect_set_float(multiplier_param, multiplier);

	gs_blend_state_push();

	gs_blend_function_separate(obs_blend_mode_params[item->blend_type].src_color,
				   obs_blend_mode_params[item->blend_type].dst_color,
				   obs_blend_mode_params[item->blend_type].src_alpha,
				   obs_blend_mode_params[item->blend_type].dst_alpha);
	gs_blend_op(obs_blend_mode_params[item->blend_type].op);

	while (gs_effect_loop(effect, tech_name))
		obs_source_draw(tex, 0, 0, 0, 0, 0);

	gs_blend_state_pop();

	GS_DEBUG_MARKER_END();
}

/* An item's show/hide transition only runs out, and releases the item's
 * source, when it is rendered. An item drawn through a texture is skipped
 * while its source has no size (a Media Source that cleared its frame at the
 * end), so render the running transition anyway: with no size it draws
 * nothing, as for an item drawn directly. */
static void render_empty_item_transition(struct obs_scene_item *item)
{
	obs_source_t *transition = NULL;

	switch (item_render_child(item)) {
	case OBS_SCENE_RENDER_CHILD_SHOW:
		transition = item->show_transition;
		break;
	case OBS_SCENE_RENDER_CHILD_HIDE:
		transition = item->hide_transition;
		break;
	case OBS_SCENE_RENDER_CHILD_SOURCE:
		break;
	}

	if (!transition)
		return;

	obs_transition_set_size(transition, 0, 0);
	obs_source_video_render(transition);
}

static inline void render_item(struct obs_scene_item *item)
{
	GS_DEBUG_MARKER_BEGIN_FORMAT(GS_DEBUG_COLOR_ITEM, "Item: %s", obs_source_get_name(item->source));

	const bool use_texrender = item_texture_enabled(item);

	obs_source_t *const source = item->source;
	const enum gs_color_space current_space = gs_get_color_space();
	const enum gs_color_space source_space = obs_source_get_color_space(source, 1, &current_space);
	const enum gs_color_format format = gs_get_format_from_space(source_space);

	/* The texrender's format is asked for only when one is there and
	 * wanted. */
	const bool format_matches = item->item_render && use_texrender &&
				    (gs_texrender_get_format(item->item_render) == format);
	const enum obs_scene_render_texrender action =
		obs_scene_render_texrender_action(item->item_render != NULL, use_texrender, format_matches);

	if (action == OBS_SCENE_RENDER_TEXRENDER_DESTROY || action == OBS_SCENE_RENDER_TEXRENDER_RECREATE) {
		gs_texrender_destroy(item->item_render);
		item->item_render = NULL;
	}

	if (action == OBS_SCENE_RENDER_TEXRENDER_CREATE || action == OBS_SCENE_RENDER_TEXRENDER_RECREATE) {
		item->item_render = gs_texrender_create(format, GS_ZS_NONE);
	}

	if (item->item_render) {
		uint32_t width = obs_source_get_width(item->source);
		uint32_t height = obs_source_get_height(item->source);

		if (obs_scene_render_size_empty(width, height)) {
			render_empty_item_transition(item);
			goto cleanup;
		}

		uint32_t cx = calc_cx(item, width);
		uint32_t cy = calc_cy(item, height);

		if (cx && cy && gs_texrender_begin_with_color_space(item->item_render, cx, cy, source_space)) {
			float cx_scale = (float)width / (float)cx;
			float cy_scale = (float)height / (float)cy;
			struct vec4 clear_color;

			/* White for Darken (#1406), see
			 * obs_scene_render_clear_white(). */
			if (obs_scene_render_clear_white(item->blend_type))
				vec4_set(&clear_color, 1.0f, 1.0f, 1.0f, 1.0f);
			else
				vec4_zero(&clear_color);
			gs_clear(GS_CLEAR_COLOR, &clear_color, 0.0f, 0);
			gs_ortho(0.0f, (float)width, 0.0f, (float)height, -100.0f, 100.0f);

			gs_matrix_scale3f(cx_scale, cy_scale, 1.0f);
			gs_matrix_translate3f(-obs_scene_crop_offset(item->crop.left, item->bounds_crop.left),
					      -obs_scene_crop_offset(item->crop.top, item->bounds_crop.top), 0.0f);

			switch (item_render_child(item)) {
			case OBS_SCENE_RENDER_CHILD_SHOW: {
				const int show_cx = obs_source_get_width(item->source);
				const int show_cy = obs_source_get_height(item->source);
				obs_transition_set_size(item->show_transition, show_cx, show_cy);
				obs_source_video_render(item->show_transition);
				break;
			}
			case OBS_SCENE_RENDER_CHILD_HIDE: {
				const int hide_cx = obs_source_get_width(item->source);
				const int hide_cy = obs_source_get_height(item->source);
				obs_transition_set_size(item->hide_transition, hide_cx, hide_cy);
				obs_source_video_render(item->hide_transition);
				break;
			}
			case OBS_SCENE_RENDER_CHILD_SOURCE:
				obs_source_set_texcoords_centered(item->source, true);
				obs_source_video_render(item->source);
				obs_source_set_texcoords_centered(item->source, false);
				break;
			}

			gs_texrender_end(item->item_render);
		}
	}

	const bool linear_srgb = obs_scene_render_linear_srgb(item->item_render != NULL, item->blend_method);
	const bool previous = gs_set_linear_srgb(linear_srgb);
	gs_matrix_push();
	gs_matrix_mul(&item->draw_transform);
	if (item->item_render) {
		render_item_texture(item, current_space, source_space);
	} else {
		switch (item_render_child(item)) {
		case OBS_SCENE_RENDER_CHILD_SHOW: {
			const int cx = obs_source_get_width(item->source);
			const int cy = obs_source_get_height(item->source);
			obs_transition_set_size(item->show_transition, cx, cy);
			obs_source_video_render(item->show_transition);
			break;
		}
		case OBS_SCENE_RENDER_CHILD_HIDE: {
			const int cx = obs_source_get_width(item->source);
			const int cy = obs_source_get_height(item->source);
			obs_transition_set_size(item->hide_transition, cx, cy);
			obs_source_video_render(item->hide_transition);
			break;
		}
		case OBS_SCENE_RENDER_CHILD_SOURCE: {
			const bool centered = obs_scene_texcoords_centered(&item->draw_transform);
			obs_source_set_texcoords_centered(item->source, centered);
			obs_source_video_render(item->source);
			obs_source_set_texcoords_centered(item->source, false);
			break;
		}
		}
	}
	gs_matrix_pop();
	gs_set_linear_srgb(previous);

cleanup:
	GS_DEBUG_MARKER_END();
}

static void scene_video_tick(void *data, float seconds)
{
	struct obs_scene *scene = data;
	struct obs_scene_item *item;

	video_lock(scene);
	item = scene->first_item;
	while (item) {
		if (item->item_render)
			gs_texrender_reset(item->item_render);
		item = item->next;
	}
	video_unlock(scene);

	/* Showing or hiding an item whose source has audio is queued as an
	 * audio action, which the audio thread applies on a sample boundary.
	 * The audio thread only renders the active tree of the output
	 * channels, so a scene that is only showing (the Studio Mode preview,
	 * a projector, the multiview) would keep its items visible forever.
	 * It mixes no audio, so apply its actions now. */
	if (!obs_source_active(scene->source)) {
		audio_lock(scene);
		for (item = scene->first_item; item; item = item->next)
			apply_scene_item_audio_actions(item, NULL, 0, 0);
		audio_unlock(scene);
	}

	UNUSED_PARAMETER(seconds);
}

/* assumes video lock */
static void update_transforms_and_prune_sources(obs_scene_t *scene, obs_scene_item_ptr_array_t *remove_items,
						obs_sceneitem_t *group_sceneitem, bool scene_size_changed)
{
	struct obs_scene_item *item = scene->first_item;
	bool rebuild_group = group_sceneitem && os_atomic_load_bool(&group_sceneitem->update_group_resize);

	while (item) {
		if (obs_source_removed(item->source)) {
			struct obs_scene_item *del_item = item;
			item = item->next;

			obs_sceneitem_remove_internal(del_item);
			da_push_back(*remove_items, &del_item);
			rebuild_group = true;
			continue;
		}

		if (item->is_group) {
			obs_scene_t *group_scene = item->source->context.data;

			video_lock(group_scene);
			update_transforms_and_prune_sources(group_scene, remove_items, item, scene_size_changed);
			video_unlock(group_scene);
		}

		if (os_atomic_load_bool(&item->update_transform) || source_size_changed(item) || scene_size_changed) {

			update_item_transform(item, true);
			rebuild_group = true;
		}

		item = item->next;
	}

	if (rebuild_group && group_sceneitem)
		resize_group(group_sceneitem, scene_size_changed);
}

static inline bool scene_size_changed(obs_scene_t *scene)
{
	uint32_t width = scene_getwidth(scene);
	uint32_t height = scene_getheight(scene);

	if (width == scene->last_width && height == scene->last_height)
		return false;

	scene->last_width = width;
	scene->last_height = height;
	return true;
}

static void scene_video_render(void *data, gs_effect_t *effect)
{
	obs_scene_item_ptr_array_t remove_items;
	struct obs_scene *scene = data;
	struct obs_scene_item *item;

	da_init(remove_items);

	video_lock(scene);

	if (!scene->is_group) {
		bool size_changed = scene_size_changed(scene);
		update_transforms_and_prune_sources(scene, &remove_items, NULL, size_changed);
	}

	gs_blend_state_push();
	gs_reset_blend_state();

	item = scene->first_item;
	while (item) {
		if (item->user_visible || scene_transition_active(item->hide_transition))
			render_item(item);

		item = item->next;
	}

	gs_blend_state_pop();

	video_unlock(scene);

	for (size_t i = 0; i < remove_items.num; i++)
		obs_sceneitem_release(remove_items.array[i]);
	da_free(remove_items);

	UNUSED_PARAMETER(effect);
}

static void set_visibility(struct obs_scene_item *item, bool vis)
{
	pthread_mutex_lock(&item->actions_mutex);

	da_resize(item->audio_actions, 0);

	if (os_atomic_load_long(&item->active_refs) > 0) {
		if (!vis)
			obs_source_remove_active_child(item->parent->source, item->source);
	} else if (vis) {
		obs_source_add_active_child(item->parent->source, item->source);
	}

	os_atomic_set_long(&item->active_refs, vis ? 1 : 0);
	item->visible = vis;
	item->user_visible = vis;

	pthread_mutex_unlock(&item->actions_mutex);
}

static obs_sceneitem_t *obs_scene_add_internal(obs_scene_t *scene, obs_source_t *source, obs_sceneitem_t *insert_after,
					       int64_t id);

static void scene_load_item(struct obs_scene *scene, obs_data_t *item_data)
{
	const char *name = obs_data_get_string(item_data, "name");
	const char *src_uuid = obs_data_get_string(item_data, "source_uuid");
	obs_source_t *source = NULL;
	const char *scale_filter_str;
	const char *blend_method_str;
	const char *blend_str;
	struct obs_scene_item *item;
	struct calldata params;
	uint8_t stack[128];
	bool visible;
	bool lock;

	if (obs_data_get_bool(item_data, "group_item_backup"))
		return;

	if (src_uuid && strlen(src_uuid) == UUID_STR_LENGTH)
		source = obs_get_source_by_uuid(src_uuid);

	/* Fall back to name if UUID was not found or is not set. */
	if (!source)
		source = obs_get_source_by_name(name);

	if (!source) {
		blog(LOG_WARNING,
		     "[scene_load_item] Source %s not "
		     "found!",
		     name);
		return;
	}

	item = obs_scene_add_internal(scene, source, NULL, obs_data_get_int(item_data, "id"));
	if (!item) {
		blog(LOG_WARNING,
		     "[scene_load_item] Could not add source '%s' "
		     "to scene '%s'!",
		     name, obs_source_get_name(scene->source));

		obs_source_release(source);
		return;
	}

	calldata_init_fixed(&params, stack, sizeof(stack));
	calldata_set_ptr(&params, "scene", scene);
	calldata_set_ptr(&params, "item", item);
	signal_handler_signal(scene->source->context.signals, "item_add", &params);

	item->is_group = strcmp(source->info.id, group_info.id) == 0;

	obs_data_set_default_int(item_data, "align", OBS_ALIGN_TOP | OBS_ALIGN_LEFT);

	item->rot = (float)obs_data_get_double(item_data, "rot");
	item->align = (uint32_t)obs_data_get_int(item_data, "align");
	visible = obs_data_get_bool(item_data, "visible");
	lock = obs_data_get_bool(item_data, "locked");

	/* Asked in this order, then passed to the choice
	 * (obs-scene-item-names.c). Each is a plain lookup in item_data; the
	 * old && chain skipped the later ones when an earlier test failed. */
	bool has_pos_rel = obs_data_has_user_value(item_data, "pos_rel");
	bool has_scale_rel = obs_data_has_user_value(item_data, "scale_rel");
	bool has_scale_ref = obs_data_has_user_value(item_data, "scale_ref");
	enum obs_scene_item_keys keys =
		obs_scene_item_transform_keys(item->absolute_coordinates, has_pos_rel, has_scale_rel, has_scale_ref);

	if (keys == OBS_SCENE_ITEM_KEYS_RELATIVE) {
		obs_data_get_vec2(item_data, "pos_rel", &item->pos);
		obs_data_get_vec2(item_data, "scale_rel", &item->scale);
		obs_data_get_vec2(item_data, "scale_ref", &item->scale_ref);
	} else {
		obs_data_get_vec2(item_data, "pos", &item->pos);
		if (keys == OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE)
			pos_from_absolute(&item->pos, &item->pos, item);
		obs_data_get_vec2(item_data, "scale", &item->scale);
	}

	obs_data_release(item->private_settings);
	item->private_settings = obs_data_get_obj(item_data, "private_settings");
	if (!item->private_settings)
		item->private_settings = obs_data_create();

	set_visibility(item, visible);
	obs_sceneitem_set_locked(item, lock);

	item->bounds_type = valid_bounds_type(obs_data_get_int(item_data, "bounds_type"));
	item->bounds_align = (uint32_t)obs_data_get_int(item_data, "bounds_align");
	item->crop_to_bounds = obs_data_get_bool(item_data, "bounds_crop");
	obs_data_get_vec2(item_data, "bounds", &item->bounds);

	bool has_bounds_rel = obs_data_has_user_value(item_data, "bounds_rel");
	keys = obs_scene_item_bounds_keys(item->absolute_coordinates, has_bounds_rel);

	if (keys == OBS_SCENE_ITEM_KEYS_RELATIVE) {
		obs_data_get_vec2(item_data, "bounds_rel", &item->bounds);
	} else {
		obs_data_get_vec2(item_data, "bounds", &item->bounds);
		if (keys == OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE)
			size_from_absolute(&item->bounds, &item->bounds, item);
	}

	/* Through the setter, which clamps a negative crop to 0: a scene
	 * collection OBS did not write can hold any value. */
	struct obs_sceneitem_crop crop = {
		.left = obs_scene_crop_from_int64(obs_data_get_int(item_data, "crop_left")),
		.top = obs_scene_crop_from_int64(obs_data_get_int(item_data, "crop_top")),
		.right = obs_scene_crop_from_int64(obs_data_get_int(item_data, "crop_right")),
		.bottom = obs_scene_crop_from_int64(obs_data_get_int(item_data, "crop_bottom")),
	};
	obs_sceneitem_set_crop(item, &crop);

	scale_filter_str = obs_data_get_string(item_data, "scale_filter");
	item->scale_filter = (enum obs_scale_type)obs_scene_item_scale_filter_from_name(scale_filter_str, astrcmpi);

	blend_method_str = obs_data_get_string(item_data, "blend_method");
	item->blend_method =
		(enum obs_blending_method)obs_scene_item_blend_method_from_name(blend_method_str, astrcmpi);

	blend_str = obs_data_get_string(item_data, "blend_type");
	item->blend_type = (enum obs_blending_type)obs_scene_item_blend_type_from_name(blend_str, astrcmpi);

	obs_data_t *show_data = obs_data_get_obj(item_data, "show_transition");
	if (show_data) {
		obs_sceneitem_transition_load(item, show_data, true);
		obs_data_release(show_data);
	}

	obs_data_t *hide_data = obs_data_get_obj(item_data, "hide_transition");
	if (hide_data) {
		obs_sceneitem_transition_load(item, hide_data, false);
		obs_data_release(hide_data);
	}

	obs_source_release(source);

	update_item_transform(item, false);
}

static void scene_load(void *data, obs_data_t *settings)
{
	struct obs_scene *scene = data;
	obs_data_array_t *items = obs_data_get_array(settings, "items");
	size_t count, i;

	remove_all_items(scene);

	if (obs_data_get_bool(settings, "custom_size")) {
		scene->cx = (uint32_t)obs_data_get_int(settings, "cx");
		scene->cy = (uint32_t)obs_data_get_int(settings, "cy");
		scene->custom_size = true;
	}

	if (obs_data_has_user_value(settings, "id_counter"))
		scene->id_counter = obs_data_get_int(settings, "id_counter");

	scene->absolute_coordinates = obs_data_get_bool(obs->data.private_data, "AbsoluteCoordinates");

	if (!items)
		return;

	count = obs_data_array_count(items);

	for (i = 0; i < count; i++) {
		obs_data_t *item_data = obs_data_array_item(items, i);
		scene_load_item(scene, item_data);
		obs_data_release(item_data);
	}

	obs_data_array_release(items);
}

static void scene_save(void *data, obs_data_t *settings);

static void scene_save_item(obs_data_array_t *array, struct obs_scene_item *item, struct obs_scene_item *backup_group)
{
	obs_data_t *item_data = obs_data_create();
	const char *name = obs_source_get_name(item->source);
	const char *src_uuid = obs_source_get_uuid(item->source);
	const char *scale_filter;
	const char *blend_method;
	const char *blend_type;
	struct vec2 pos = item->pos;
	struct vec2 scale = item->scale;
	float rot = item->rot;

	if (backup_group) {
		get_ungrouped_transform(backup_group, item, &pos, &scale, &rot);
	}

	obs_data_set_string(item_data, "name", name);
	obs_data_set_string(item_data, "source_uuid", src_uuid);
	obs_data_set_bool(item_data, "visible", item->user_visible);
	obs_data_set_bool(item_data, "locked", item->locked);
	obs_data_set_double(item_data, "rot", rot);
	obs_data_set_vec2(item_data, "scale_ref", &item->scale_ref);
	obs_data_set_int(item_data, "align", (int)item->align);
	obs_data_set_int(item_data, "bounds_type", (int)item->bounds_type);
	obs_data_set_int(item_data, "bounds_align", (int)item->bounds_align);
	obs_data_set_bool(item_data, "bounds_crop", item->crop_to_bounds);
	obs_data_set_int(item_data, "crop_left", (int)item->crop.left);
	obs_data_set_int(item_data, "crop_top", (int)item->crop.top);
	obs_data_set_int(item_data, "crop_right", (int)item->crop.right);
	obs_data_set_int(item_data, "crop_bottom", (int)item->crop.bottom);
	obs_data_set_int(item_data, "id", item->id);
	obs_data_set_bool(item_data, "group_item_backup", !!backup_group);

	if (obs_scene_item_save_relative(item->absolute_coordinates)) {
		/* For backwards compatibility, also store absolute values. */
		struct vec2 tmp_abs;

		pos_to_absolute(&tmp_abs, &pos, item);
		obs_data_set_vec2(item_data, "pos", &tmp_abs);
		obs_data_set_vec2(item_data, "pos_rel", &pos);

		item_canvas_scale(&tmp_abs, item);
		obs_data_set_vec2(item_data, "scale", &tmp_abs);
		obs_data_set_vec2(item_data, "scale_rel", &scale);

		size_to_absolute(&tmp_abs, &item->bounds, item);
		obs_data_set_vec2(item_data, "bounds", &tmp_abs);
		obs_data_set_vec2(item_data, "bounds_rel", &item->bounds);
	} else {
		obs_data_set_vec2(item_data, "pos", &pos);
		obs_data_set_vec2(item_data, "scale", &scale);
		obs_data_set_vec2(item_data, "bounds", &item->bounds);
	}

	if (item->is_group) {
		obs_scene_t *group_scene = item->source->context.data;
		obs_sceneitem_t *group_item;

		/* save group items as part of main scene, but ignored.
		 * causes an automatic ungroup if scene collection file
		 * is loaded in previous versions. */
		full_lock(group_scene);

		group_item = group_scene->first_item;
		while (group_item) {
			scene_save_item(array, group_item, item);
			group_item = group_item->next;
		}

		full_unlock(group_scene);
	}

	scale_filter = obs_scene_item_scale_filter_name((int)item->scale_filter);
	obs_data_set_string(item_data, "scale_filter", scale_filter);

	blend_method = obs_scene_item_blend_method_name((int)item->blend_method);
	obs_data_set_string(item_data, "blend_method", blend_method);

	blend_type = obs_scene_item_blend_type_name((int)item->blend_type);
	obs_data_set_string(item_data, "blend_type", blend_type);

	obs_data_t *show_data = obs_sceneitem_transition_save(item, true);
	obs_data_set_obj(item_data, "show_transition", show_data);
	obs_data_release(show_data);

	obs_data_t *hide_data = obs_sceneitem_transition_save(item, false);
	obs_data_set_obj(item_data, "hide_transition", hide_data);
	obs_data_release(hide_data);

	obs_data_set_obj(item_data, "private_settings", item->private_settings);

	obs_data_array_push_back(array, item_data);
	obs_data_release(item_data);
}

static void scene_save(void *data, obs_data_t *settings)
{
	struct obs_scene *scene = data;
	obs_data_array_t *array = obs_data_array_create();
	struct obs_scene_item *item;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		scene_save_item(array, item, NULL);
		item = item->next;
	}

	obs_data_set_int(settings, "id_counter", scene->id_counter);
	obs_data_set_bool(settings, "custom_size", scene->custom_size);
	if (scene->custom_size) {
		obs_data_set_int(settings, "cx", scene->cx);
		obs_data_set_int(settings, "cy", scene->cy);
	}

	full_unlock(scene);

	obs_data_set_array(settings, "items", array);
	obs_data_array_release(array);
}

static uint32_t canvas_getwidth(obs_weak_canvas_t *weak)
{
	uint32_t width = 0;

	obs_canvas_t *canvas = obs_weak_canvas_get_canvas(weak);
	if (canvas) {
		width = canvas->ovi.base_width;
		obs_canvas_release(canvas);
	}

	return width;
}

static uint32_t canvas_getheight(obs_weak_canvas_t *weak)
{
	uint32_t height = 0;

	obs_canvas_t *canvas = obs_weak_canvas_get_canvas(weak);
	if (canvas) {
		height = canvas->ovi.base_height;
		obs_canvas_release(canvas);
	}

	return height;
}

static uint32_t scene_getwidth(void *data)
{
	obs_scene_t *scene = data;
	if (scene->custom_size)
		return scene->cx;
	if (scene->source->canvas)
		return canvas_getwidth(scene->source->canvas);
	if (obs->data.main_canvas->mix)
		return obs->data.main_canvas->mix->ovi.base_width;
	return 0;
}

static uint32_t scene_getheight(void *data)
{
	obs_scene_t *scene = data;
	if (scene->custom_size)
		return scene->cy;
	if (scene->source->canvas)
		return canvas_getheight(scene->source->canvas);
	if (obs->data.main_canvas->mix)
		return obs->data.main_canvas->mix->ovi.base_height;
	return 0;
}

/* apply_scene_item_audio_actions(), apply_scene_item_volume() and
 * process_all_audio_actions() are in obs-scene-item-audio-actions.c. */

static void mix_audio_with_buf(float *p_out, float *p_in, float *buf_in, size_t pos, size_t count)
{
	register float *out = p_out + pos;
	register float *buf = buf_in;
	register float *in = p_in;
	register float *end = in + count;

	while (in < end)
		*(out++) += *(in++) * *(buf++);
}

static inline void scene_mix_audio(float *p_out, float *p_in, size_t pos, size_t count)
{
	register float *out = p_out + pos;
	register float *in = p_in;
	register float *end = in + count;

	while (in < end)
		*(out++) += *(in++);
}

/* The window decisions are obs-scene-audio-window.c.h. The scene still takes
 * audio_lock, calls apply_scene_item_volume() and
 * process_all_audio_actions(), asks the sources for their pending state,
 * timestamp and mix, and mixes. It reads each item and source field only
 * where the old expressions read it, in their order. */

/* The source an item's audio is read from. The reads of the old
 * `visible && active(show) ? show : !visible && active(hide) ? hide : source`:
 * visible, the show transition when visible, then, when it is not picked,
 * visible again and the hide transition when not visible. */
static inline struct obs_source *audio_window_source(struct obs_scene_item *item)
{
	bool visible = item->visible;
	bool show_active = false;
	bool hide_active = false;

	if (visible)
		show_active = scene_transition_active(item->show_transition);
	if (!show_active) {
		visible = item->visible;
		if (!visible)
			hide_active = scene_transition_active(item->hide_transition);
	}

	switch (obs_scene_audio_window_pick(visible, show_active, hide_active)) {
	case OBS_SCENE_AUDIO_WINDOW_SHOW:
		return item->show_transition;
	case OBS_SCENE_AUDIO_WINDOW_HIDE:
		return item->hide_transition;
	case OBS_SCENE_AUDIO_WINDOW_SOURCE:
		break;
	}
	return item->source;
}

/* The reads of the old `item->visible || active(hide)`: visible, and the
 * hide transition only when not visible. */
static inline void audio_window_heard(const struct obs_scene_item *item, bool *visible, bool *hide_active)
{
	*visible = item->visible;
	if (!*visible)
		*hide_active = scene_transition_active(item->hide_transition);
}

static bool scene_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio_output, uint32_t mixers,
			       size_t channels, size_t sample_rate)
{
	uint64_t timestamp = 0;
	float buf[AUDIO_OUTPUT_FRAMES];
	struct obs_source_audio_mix child_audio;
	struct obs_scene *scene = data;
	struct obs_scene_item *item;

	audio_lock(scene);

	item = scene->first_item;
	while (item) {
		struct obs_source *source = audio_window_source(item);
		const bool pending = obs_source_audio_pending(source);
		bool visible = false;
		bool hide_active = false;

		if (!pending)
			audio_window_heard(item, &visible, &hide_active);

		if (obs_scene_audio_window_counts(pending, visible, hide_active))
			timestamp = obs_scene_audio_window_earliest(timestamp, obs_source_get_audio_timestamp(source));

		item = item->next;
	}

	if (obs_scene_audio_window_silent(timestamp)) {
		/* just process all pending audio actions if no audio playing,
		 * otherwise audio actions will just never be processed */
		item = scene->first_item;
		while (item) {
			process_all_audio_actions(item, sample_rate);
			item = item->next;
		}

		audio_unlock(scene);
		return false;
	}

	item = scene->first_item;
	while (item) {
		struct obs_scene_audio_window window;
		uint64_t source_ts = 0;
		bool apply_buf;
		bool pending;
		bool visible = false;
		bool hide_active = false;
		struct obs_source *source = audio_window_source(item);

		apply_buf = apply_scene_item_volume(item, buf, timestamp, sample_rate);

		pending = obs_source_audio_pending(source);
		if (!pending)
			source_ts = obs_source_get_audio_timestamp(source);

		window = obs_scene_audio_window_frames(pending, source_ts, timestamp, sample_rate);
		if (!window.mix) {
			item = item->next;
			continue;
		}

		if (!apply_buf)
			audio_window_heard(item, &visible, &hide_active);

		if (!obs_scene_audio_window_mixes(apply_buf, visible, hide_active)) {
			item = item->next;
			continue;
		}

		obs_source_get_audio_mix(source, &child_audio);

		if (obs_scene_audio_window_adds(source->audio_is_duplicated)) {
			for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
				if ((mixers & (1 << mix)) == 0)
					continue;

				for (size_t ch = 0; ch < channels; ch++) {
					float *out = audio_output->output[mix].data[ch];
					float *in = child_audio.output[mix].data[ch];
					if (apply_buf)
						mix_audio_with_buf(out, in, buf, window.pos, window.count);
					else
						scene_mix_audio(out, in, window.pos, window.count);
				}
			}
		}

		item = item->next;
	}

	*ts_out = timestamp;
	audio_unlock(scene);

	return true;
}

enum gs_color_space scene_video_get_color_space(void *data, size_t count, const enum gs_color_space *preferred_spaces)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(count);
	UNUSED_PARAMETER(preferred_spaces);

	enum gs_color_space space = GS_CS_SRGB;
	struct obs_video_info ovi;
	if (obs_get_video_info(&ovi)) {
		if (ovi.colorspace == VIDEO_CS_2100_PQ || ovi.colorspace == VIDEO_CS_2100_HLG)
			space = GS_CS_709_EXTENDED;
	}

	return space;
}

const struct obs_source_info scene_info = {
	.id = "scene",
	.type = OBS_SOURCE_TYPE_SCENE,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW | OBS_SOURCE_COMPOSITE | OBS_SOURCE_DO_NOT_DUPLICATE |
			OBS_SOURCE_SRGB | OBS_SOURCE_REQUIRES_CANVAS,
	.get_name = scene_getname,
	.create = scene_create,
	.destroy = scene_destroy,
	.video_tick = scene_video_tick,
	.video_render = scene_video_render,
	.audio_render = scene_audio_render,
	.get_width = scene_getwidth,
	.get_height = scene_getheight,
	.load = scene_load,
	.save = scene_save,
	.enum_active_sources = scene_enum_active_sources,
	.enum_all_sources = scene_enum_all_sources,
	.video_get_color_space = scene_video_get_color_space,
};

const struct obs_source_info group_info = {
	.id = "group",
	.type = OBS_SOURCE_TYPE_SCENE,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW | OBS_SOURCE_COMPOSITE | OBS_SOURCE_SRGB |
			OBS_SOURCE_REQUIRES_CANVAS,
	.get_name = group_getname,
	.create = scene_create,
	.destroy = scene_destroy,
	.video_tick = scene_video_tick,
	.video_render = scene_video_render,
	.audio_render = scene_audio_render,
	.get_width = scene_getwidth,
	.get_height = scene_getheight,
	.load = scene_load,
	.save = scene_save,
	.enum_active_sources = scene_enum_active_sources,
	.enum_all_sources = scene_enum_all_sources,
	.video_get_color_space = scene_video_get_color_space,
};

static inline obs_scene_t *create_id(obs_canvas_t *canvas, const char *id, const char *name)
{
	struct obs_source *source = obs_source_create_canvas(canvas, id, name, NULL, NULL);
	return source->context.data;
}

static inline obs_scene_t *create_private_id(obs_canvas_t *canvas, const char *id, const char *name)
{
	struct obs_source *source = obs_source_create_private_with_canvas(canvas, id, name, NULL);
	return source->context.data;
}

obs_scene_t *obs_scene_create(const char *name)
{
	return create_id(obs->data.main_canvas, "scene", name);
}

obs_scene_t *obs_scene_create_private(const char *name)
{
	return create_private_id(obs->data.main_canvas, "scene", name);
}

static obs_source_t *get_child_at_idx(obs_scene_t *scene, size_t idx)
{
	struct obs_scene_item *item = scene->first_item;

	while (item && idx--)
		item = item->next;
	return item ? item->source : NULL;
}

static inline obs_source_t *dup_child(obs_scene_item_ptr_array_t *old_items, size_t idx, obs_scene_t *new_scene,
				      bool private)
{
	obs_source_t *source;

	source = old_items->array[idx]->source;

	/* if the old item is referenced more than once in the old scene,
	 * make sure they're referenced similarly in the new scene to reduce
	 * load times */
	for (size_t i = 0; i < idx; i++) {
		struct obs_scene_item *item = old_items->array[i];
		if (item->source == source) {
			source = get_child_at_idx(new_scene, i);
			return obs_source_get_ref(source);
		}
	}

	return obs_source_duplicate(source, private ? obs_source_get_name(source) : NULL, private);
}

/* A group is copied with the duplicate type asked for, so that a private copy
 * of a scene (Studio Mode with "Duplicate Sources") also gets private copies of
 * the sources inside its groups. */
static inline obs_source_t *dup_group(obs_source_t *group, enum obs_scene_duplicate_type type, bool private)
{
	obs_scene_t *new_group =
		obs_scene_duplicate(obs_group_from_source(group), private ? obs_source_get_name(group) : NULL, type);
	return obs_scene_get_source(new_group);
}

static inline obs_source_t *new_ref(obs_source_t *source)
{
	return obs_source_get_ref(source);
}

static inline void duplicate_item_data(struct obs_scene_item *dst, struct obs_scene_item *src,
				       bool defer_texture_update, bool duplicate_hotkeys)
{
	struct obs_scene *dst_scene = dst->parent;

	if (!src->user_visible)
		set_visibility(dst, false);

	dst->selected = src->selected;
	dst->pos = src->pos;
	dst->rot = src->rot;
	dst->scale = src->scale;
	/* The scale is relative to the canvas size it was set at. */
	dst->scale_ref = src->scale_ref;
	dst->align = src->align;
	dst->last_width = src->last_width;
	dst->last_height = src->last_height;
	dst->output_scale = src->output_scale;
	dst->scale_filter = src->scale_filter;
	dst->blend_method = src->blend_method;
	dst->blend_type = src->blend_type;
	dst->box_transform = src->box_transform;
	dst->box_scale = src->box_scale;
	dst->draw_transform = src->draw_transform;
	dst->bounds_type = src->bounds_type;
	dst->bounds_align = src->bounds_align;
	dst->bounds = src->bounds;
	dst->crop_to_bounds = src->crop_to_bounds;
	dst->bounds_crop = src->bounds_crop;

	if (src->show_transition) {
		obs_source_t *transition =
			obs_source_duplicate(src->show_transition, obs_source_get_name(src->show_transition), true);
		obs_sceneitem_set_transition(dst, true, transition);
		obs_source_release(transition);
	}
	if (src->hide_transition) {
		obs_source_t *transition =
			obs_source_duplicate(src->hide_transition, obs_source_get_name(src->hide_transition), true);
		obs_sceneitem_set_transition(dst, false, transition);
		obs_source_release(transition);
	}
	dst->show_transition_duration = src->show_transition_duration;
	dst->hide_transition_duration = src->hide_transition_duration;

	if (duplicate_hotkeys && !dst_scene->source->context.private) {
		struct dstr show = {0};
		struct dstr hide = {0};
		obs_data_array_t *data0 = NULL;
		obs_data_array_t *data1 = NULL;

		obs_hotkey_pair_save(src->toggle_visibility, &data0, &data1);
		obs_hotkey_pair_load(dst->toggle_visibility, data0, data1);

		/* Fix scene item ID */
		dstr_printf(&show, "libobs.show_scene_item.%" PRIi64, dst->id);
		dstr_printf(&hide, "libobs.hide_scene_item.%" PRIi64, dst->id);
		obs_hotkey_pair_set_names(dst->toggle_visibility, show.array, hide.array);

		obs_data_array_release(data0);
		obs_data_array_release(data1);
		dstr_free(&show);
		dstr_free(&hide);
	}

	obs_sceneitem_set_crop(dst, &src->crop);
	obs_sceneitem_set_locked(dst, src->locked);

	if (defer_texture_update) {
		os_atomic_set_bool(&dst->update_transform, true);
	}

	obs_data_apply(dst->private_settings, src->private_settings);
}

obs_scene_t *obs_scene_duplicate(obs_scene_t *scene, const char *name, enum obs_scene_duplicate_type type)
{
	bool make_unique = type == OBS_SCENE_DUP_COPY || type == OBS_SCENE_DUP_PRIVATE_COPY;
	bool make_private = type == OBS_SCENE_DUP_PRIVATE_REFS || type == OBS_SCENE_DUP_PRIVATE_COPY;
	obs_scene_item_ptr_array_t items;
	struct obs_scene *new_scene;
	struct obs_scene_item *item;
	struct obs_source *source;

	da_init(items);

	if (!obs_ptr_valid(scene, "obs_scene_duplicate"))
		return NULL;

	/* --------------------------------- */

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		da_push_back(items, &item);
		obs_sceneitem_addref(item);
		item = item->next;
	}

	full_unlock(scene);

	/* --------------------------------- */

	obs_canvas_t *canvas = obs_weak_canvas_get_canvas(scene->source->canvas);
	new_scene = make_private ? create_private_id(canvas, scene->source->info.id, name)
				 : create_id(canvas, scene->source->info.id, name);
	obs_canvas_release(canvas);

	new_scene->is_group = scene->is_group;
	new_scene->custom_size = scene->custom_size;
	new_scene->cx = scene->cx;
	new_scene->cy = scene->cy;
	new_scene->absolute_coordinates = scene->absolute_coordinates;
	new_scene->last_width = scene->last_width;
	new_scene->last_height = scene->last_height;

	obs_source_copy_filters(new_scene->source, scene->source);

	obs_data_apply(new_scene->source->private_settings, scene->source->private_settings);

	/* A group's sub-items are copied only for a private copy (Studio Mode
	 * with "Duplicate Sources"), which must not share sources with the
	 * scene being edited. A public copy of a group keeps its sources. */
	if (scene->is_group && type != OBS_SCENE_DUP_PRIVATE_COPY)
		make_unique = false;

	for (size_t i = 0; i < items.num; i++) {
		item = items.array[i];
		if (item->is_group)
			source = dup_group(item->source, type, make_private);
		else if (make_unique)
			source = dup_child(&items, i, new_scene, make_private);
		else
			source = new_ref(item->source);

		if (source) {
			struct obs_scene_item *new_item = obs_scene_add(new_scene, source);

			if (!new_item) {
				obs_source_release(source);
				continue;
			}

			duplicate_item_data(new_item, item, false, false);

			obs_source_release(source);
		}
	}

	for (size_t i = 0; i < items.num; i++)
		obs_sceneitem_release(items.array[i]);

	if (new_scene->is_group)
		resize_scene(new_scene);

	da_free(items);
	return new_scene;
}

static inline void obs_scene_addref(obs_scene_t *scene)
{
	if (scene)
		obs_source_addref(scene->source);
}

obs_scene_t *obs_scene_get_ref(obs_scene_t *scene)
{
	if (!scene)
		return NULL;
	if (obs_source_get_ref(scene->source) != NULL)
		return scene;
	return NULL;
}

void obs_scene_release(obs_scene_t *scene)
{
	if (scene)
		obs_source_release(scene->source);
}

obs_source_t *obs_scene_get_source(const obs_scene_t *scene)
{
	return scene ? scene->source : NULL;
}

obs_scene_t *obs_scene_from_source(const obs_source_t *source)
{
	if (!source || strcmp(source->info.id, scene_info.id) != 0)
		return NULL;

	return source->context.data;
}

obs_scene_t *obs_group_from_source(const obs_source_t *source)
{
	if (!source || strcmp(source->info.id, group_info.id) != 0)
		return NULL;

	return source->context.data;
}

obs_sceneitem_t *obs_scene_find_source(obs_scene_t *scene, const char *name)
{
	struct obs_scene_item *item;

	if (!scene)
		return NULL;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		if (strcmp(item->source->context.name, name) == 0)
			break;

		item = item->next;
	}

	full_unlock(scene);

	return item;
}

obs_sceneitem_t *obs_scene_find_source_recursive(obs_scene_t *scene, const char *name)
{
	struct obs_scene_item *item;

	if (!scene)
		return NULL;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		if (strcmp(item->source->context.name, name) == 0)
			break;

		if (item->is_group) {
			obs_scene_t *group = item->source->context.data;
			obs_sceneitem_t *child = obs_scene_find_source(group, name);
			if (child) {
				item = child;
				break;
			}
		}

		item = item->next;
	}

	full_unlock(scene);

	return item;
}

obs_sceneitem_t *obs_scene_find_sceneitem_by_id(obs_scene_t *scene, int64_t id)
{
	struct obs_scene_item *item;

	if (!scene)
		return NULL;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		if (item->id == id)
			break;

		item = item->next;
	}

	full_unlock(scene);

	return item;
}

/* Must be called with the scene locked. Only compares pointers and follows the
 * links of items that are in the scene, which the scene lock protects. An item
 * that left the scene may be changed by whoever holds its new scene's lock, so
 * its fields must not be read here. */
static bool scene_has_item(const struct obs_scene *scene, const struct obs_scene_item *item)
{
	for (const struct obs_scene_item *cur = scene->first_item; cur; cur = cur->next) {
		if (cur == item)
			return true;
	}
	return false;
}

void obs_scene_enum_items(obs_scene_t *scene, bool (*callback)(obs_scene_t *, obs_sceneitem_t *, void *), void *param)
{
	struct obs_scene_item *item;
	obs_scene_item_ptr_array_t items;

	if (!scene || !callback)
		return;

	da_init(items);

	full_lock(scene);

	/* The callback may remove items other than the current one, or move them
	 * to another scene (a group), so walk a snapshot that holds a reference
	 * to every item and skip the ones that are no longer in this scene.
	 * Membership is decided by the scene's own item list, which the scene
	 * lock we hold protects; the fields of an item that has left the scene
	 * are not read. An item being removed stays in the list until it is
	 * detached, but it is already marked removed (also while its removal
	 * signals run), and its removal needs this scene's lock. */
	item = scene->first_item;
	while (item) {
		obs_sceneitem_addref(item);
		da_push_back(items, &item);
		item = item->next;
	}

	for (size_t i = 0; i < items.num; i++) {
		item = items.array[i];

		if (!scene_has_item(scene, item) || item->removed)
			continue;

		if (!callback(scene, item, param))
			break;
	}

	for (size_t i = 0; i < items.num; i++)
		obs_sceneitem_release(items.array[i]);

	da_free(items);

	full_unlock(scene);
}

static obs_sceneitem_t *sceneitem_get_ref(obs_sceneitem_t *si)
{
	long owners = os_atomic_load_long(&si->ref);
	while (owners > 0) {
		if (os_atomic_compare_exchange_long(&si->ref, &owners, owners + 1)) {
			return si;
		}
	}
	return NULL;
}

static bool hotkey_show_sceneitem(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);

	obs_sceneitem_t *si = sceneitem_get_ref(data);
	if (pressed && si && !si->user_visible) {
		obs_sceneitem_set_visible(si, true);
		obs_sceneitem_release(si);
		return true;
	}

	obs_sceneitem_release(si);
	return false;
}

static bool hotkey_hide_sceneitem(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);

	obs_sceneitem_t *si = sceneitem_get_ref(data);
	if (pressed && si && si->user_visible) {
		obs_sceneitem_set_visible(si, false);
		obs_sceneitem_release(si);
		return true;
	}

	obs_sceneitem_release(si);
	return false;
}

static void init_hotkeys(obs_scene_t *scene, obs_sceneitem_t *item, const char *name)
{
	struct obs_data_array *hotkey_array;
	obs_data_t *hotkey_data = scene->source->context.hotkey_data;

	struct dstr show = {0};
	struct dstr hide = {0};
	struct dstr legacy = {0};
	struct dstr show_desc = {0};
	struct dstr hide_desc = {0};

	dstr_printf(&show, "libobs.show_scene_item.%" PRIi64, item->id);
	dstr_printf(&hide, "libobs.hide_scene_item.%" PRIi64, item->id);

	dstr_copy(&show_desc, obs->hotkeys.sceneitem_show);
	dstr_replace(&show_desc, "%1", name);
	dstr_copy(&hide_desc, obs->hotkeys.sceneitem_hide);
	dstr_replace(&hide_desc, "%1", name);

	/* Check if legacy keys exists, migrate if necessary */
	dstr_printf(&legacy, "libobs.show_scene_item.%s", name);
	hotkey_array = obs_data_get_array(hotkey_data, legacy.array);
	if (hotkey_array) {
		obs_data_set_array(hotkey_data, show.array, hotkey_array);
		obs_data_array_release(hotkey_array);
	}

	dstr_printf(&legacy, "libobs.hide_scene_item.%s", name);
	hotkey_array = obs_data_get_array(hotkey_data, legacy.array);
	if (hotkey_array) {
		obs_data_set_array(hotkey_data, hide.array, hotkey_array);
		obs_data_array_release(hotkey_array);
	}

	item->toggle_visibility = obs_hotkey_pair_register_source(scene->source, show.array, show_desc.array,
								  hide.array, hide_desc.array, hotkey_show_sceneitem,
								  hotkey_hide_sceneitem, item, item);

	dstr_free(&show);
	dstr_free(&hide);
	dstr_free(&legacy);
	dstr_free(&show_desc);
	dstr_free(&hide_desc);
}

static void sceneitem_rename_hotkey(const obs_sceneitem_t *scene_item, const char *new_name)
{
	struct dstr show_desc = {0};
	struct dstr hide_desc = {0};

	dstr_copy(&show_desc, obs->hotkeys.sceneitem_show);
	dstr_replace(&show_desc, "%1", new_name);
	dstr_copy(&hide_desc, obs->hotkeys.sceneitem_hide);
	dstr_replace(&hide_desc, "%1", new_name);

	obs_hotkey_pair_set_descriptions(scene_item->toggle_visibility, show_desc.array, hide_desc.array);

	dstr_free(&show_desc);
	dstr_free(&hide_desc);
}

static void sceneitem_renamed(void *param, calldata_t *data)
{
	obs_sceneitem_t *scene_item = param;
	const char *name = calldata_string(data, "new_name");

	sceneitem_rename_hotkey(scene_item, name);
}

static inline bool source_has_audio(obs_source_t *source)
{
	return (source->info.output_flags & (OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE)) != 0;
}

static obs_sceneitem_t *obs_scene_add_internal(obs_scene_t *scene, obs_source_t *source, obs_sceneitem_t *insert_after,
					       int64_t id)
{
	struct obs_scene_item *last;
	struct obs_scene_item *item;
	pthread_mutex_t mutex;

	struct item_action action = {.visible = true, .timestamp = os_gettime_ns()};

	if (!scene)
		return NULL;

	source = obs_source_get_ref(source);
	if (!source) {
		blog(LOG_ERROR, "Tried to add a NULL source to a scene");
		return NULL;
	}

	if (os_atomic_load_bool(&source->removed)) {
		blog(LOG_WARNING, "Tried to add a removed source to a scene");
		goto release_source_and_fail;
	}

	if (pthread_mutex_init(&mutex, NULL) != 0) {
		blog(LOG_WARNING, "Failed to create scene item mutex");
		goto release_source_and_fail;
	}

	if (!obs_source_add_active_child(scene->source, source)) {
		blog(LOG_WARNING, "Failed to add source to scene due to "
				  "infinite source recursion");
		pthread_mutex_destroy(&mutex);
		goto release_source_and_fail;
	}

	item = bzalloc(sizeof(struct obs_scene_item));
	item->source = source;
	item->id = id ? id : ++scene->id_counter;
	item->parent = scene;
	item->ref = 1;
	item->align = OBS_ALIGN_TOP | OBS_ALIGN_LEFT;
	item->actions_mutex = mutex;
	item->user_visible = true;
	item->locked = false;
	item->is_group = strcmp(source->info.id, group_info.id) == 0;
	item->is_scene = strcmp(source->info.id, scene_info.id) == 0;
	item->private_settings = obs_data_create();
	item->toggle_visibility = OBS_INVALID_HOTKEY_PAIR_ID;
	item->absolute_coordinates = scene->absolute_coordinates;
	os_atomic_set_long(&item->active_refs, 1);
	vec2_set(&item->scale, 1.0f, 1.0f);
	get_scene_dimensions(item, &item->scale_ref.x, &item->scale_ref.y);
	matrix4_identity(&item->draw_transform);
	matrix4_identity(&item->box_transform);

	/* Ensure initial position is still top-left corner in relative mode. */
	if (!item->absolute_coordinates)
		pos_from_absolute(&item->pos, &item->pos, item);

	if (source_has_audio(source)) {
		item->visible = false;
		da_push_back(item->audio_actions, &action);
	} else {
		item->visible = true;
	}

	full_lock(scene);

	if (insert_after) {
		obs_sceneitem_t *next = insert_after->next;
		if (next)
			next->prev = item;
		item->next = insert_after->next;
		item->prev = insert_after;
		insert_after->next = item;
	} else {
		last = scene->first_item;
		if (!last) {
			scene->first_item = item;
		} else {
			while (last->next)
				last = last->next;

			last->next = item;
			item->prev = last;
		}
	}

	full_unlock(scene);

	if (!scene->source->context.private)
		init_hotkeys(scene, item, obs_source_get_name(source));

	signal_handler_connect(obs_source_get_signal_handler(source), "rename", sceneitem_renamed, item);

	return item;

release_source_and_fail:
	obs_source_release(source);
	return NULL;
}

obs_sceneitem_t *obs_scene_add(obs_scene_t *scene, obs_source_t *source)
{
	obs_sceneitem_t *item = obs_scene_add_internal(scene, source, NULL, 0);
	struct calldata params;
	uint8_t stack[128];

	if (!item)
		return NULL;

	calldata_init_fixed(&params, stack, sizeof(stack));
	calldata_set_ptr(&params, "scene", scene);
	calldata_set_ptr(&params, "item", item);
	signal_handler_signal(scene->source->context.signals, "item_add", &params);
	return item;
}

static void obs_sceneitem_destroy(obs_sceneitem_t *item)
{
	if (item) {
		if (item->item_render) {
			obs_enter_graphics();
			gs_texrender_destroy(item->item_render);
			obs_leave_graphics();
		}
		obs_data_release(item->private_settings);
		obs_hotkey_pair_unregister(item->toggle_visibility);
		pthread_mutex_destroy(&item->actions_mutex);
		signal_handler_disconnect(obs_source_get_signal_handler(item->source), "rename", sceneitem_renamed,
					  item);
		if (item->show_transition)
			obs_source_release(item->show_transition);
		if (item->hide_transition)
			obs_source_release(item->hide_transition);
		if (item->source)
			obs_source_release(item->source);
		da_free(item->audio_actions);
		bfree(item);
	}
}

void obs_sceneitem_addref(obs_sceneitem_t *item)
{
	if (item)
		os_atomic_inc_long(&item->ref);
}

void obs_sceneitem_release(obs_sceneitem_t *item)
{
	if (!item)
		return;

	if (os_atomic_dec_long(&item->ref) == 0)
		obs_sceneitem_destroy(item);
}

static void obs_sceneitem_remove_internal(obs_sceneitem_t *item)
{
	obs_scene_t *parent = item->parent;
	item->removed = true;

	obs_sceneitem_select(item, false);
	set_visibility(item, false);

	detach_sceneitem(item);
	signal_item_remove(parent, item);

	obs_sceneitem_set_transition(item, true, NULL);
	obs_sceneitem_set_transition(item, false, NULL);
}

void obs_sceneitem_remove(obs_sceneitem_t *item)
{
	obs_scene_t *scene;

	if (!item || item->removed)
		return;

	scene = item->parent;

	assert(scene != NULL);
	assert(scene->source != NULL);

	full_lock(scene);
	obs_sceneitem_remove_internal(item);
	full_unlock(scene);

	obs_sceneitem_release(item);
}

void obs_sceneitem_save(obs_sceneitem_t *item, obs_data_array_t *arr)
{
	scene_save_item(arr, item, NULL);
}

void sceneitem_restore(obs_data_t *data, void *vp)
{
	obs_scene_t *scene = (obs_scene_t *)vp;
	scene_load_item(scene, data);
}

void obs_sceneitems_add(obs_scene_t *scene, obs_data_array_t *data)
{
	obs_data_array_enum(data, sceneitem_restore, scene);
}

obs_scene_t *obs_sceneitem_get_scene(const obs_sceneitem_t *item)
{
	return item ? item->parent : NULL;
}

obs_source_t *obs_sceneitem_get_source(const obs_sceneitem_t *item)
{
	return item ? item->source : NULL;
}

static void signal_parent(obs_scene_t *parent, const char *command, calldata_t *params)
{
	calldata_set_ptr(params, "scene", parent);
	signal_handler_signal(parent->source->context.signals, command, params);
}

struct passthrough {
	obs_data_array_t *ids;
	obs_data_array_t *scenes_and_groups;
	bool all_items;
};

bool save_transform_states(obs_scene_t *scene, obs_sceneitem_t *item, void *vp_pass)
{
	struct passthrough *pass = (struct passthrough *)vp_pass;
	if (obs_sceneitem_selected(item) || pass->all_items) {
		obs_data_t *temp = obs_data_create();
		obs_data_array_t *item_ids = (obs_data_array_t *)pass->ids;

		struct obs_transform_info info;
		struct obs_sceneitem_crop crop;
		obs_sceneitem_get_info2(item, &info);
		obs_sceneitem_get_crop(item, &crop);

		struct vec2 pos = info.pos;
		struct vec2 scale = info.scale;
		float rot = info.rot;
		uint32_t alignment = info.alignment;
		uint32_t bounds_type = info.bounds_type;
		uint32_t bounds_alignment = info.bounds_alignment;
		bool crop_to_bounds = info.crop_to_bounds;
		struct vec2 bounds = info.bounds;

		obs_data_set_int(temp, "id", obs_sceneitem_get_id(item));
		obs_data_set_vec2(temp, "pos", &pos);
		obs_data_set_vec2(temp, "scale", &scale);
		obs_data_set_double(temp, "rot", rot);
		obs_data_set_int(temp, "alignment", alignment);
		obs_data_set_int(temp, "bounds_type", bounds_type);
		obs_data_set_vec2(temp, "bounds", &bounds);
		obs_data_set_int(temp, "bounds_alignment", bounds_alignment);
		obs_data_set_bool(temp, "crop_to_bounds", crop_to_bounds);
		obs_data_set_int(temp, "top", crop.top);
		obs_data_set_int(temp, "bottom", crop.bottom);
		obs_data_set_int(temp, "left", crop.left);
		obs_data_set_int(temp, "right", crop.right);

		obs_data_array_push_back(item_ids, temp);

		obs_data_release(temp);
	}

	obs_source_t *item_source = obs_sceneitem_get_source(item);

	if (obs_source_is_group(item_source)) {
		obs_data_t *temp = obs_data_create();
		obs_data_array_t *nids = obs_data_array_create();

		obs_data_set_string(temp, "scene_name", obs_source_get_name(item_source));
		obs_data_set_string(temp, "scene_uuid", obs_source_get_uuid(item_source));
		obs_data_set_bool(temp, "is_group", true);
		obs_data_set_string(temp, "group_parent", obs_source_get_uuid(obs_scene_get_source(scene)));

		struct passthrough npass = {nids, pass->scenes_and_groups, pass->all_items};
		obs_sceneitem_group_enum_items(item, save_transform_states, (void *)&npass);

		obs_data_set_array(temp, "items", nids);

		obs_data_array_push_back(pass->scenes_and_groups, temp);

		obs_data_release(temp);
		obs_data_array_release(nids);
	}

	return true;
}

obs_data_t *obs_scene_save_transform_states(obs_scene_t *scene, bool all_items)
{
	obs_data_t *wrapper = obs_data_create();
	obs_data_array_t *scenes_and_groups = obs_data_array_create();
	obs_data_array_t *item_ids = obs_data_array_create();

	struct passthrough pass = {item_ids, scenes_and_groups, all_items};

	obs_data_t *temp = obs_data_create();

	obs_data_set_string(temp, "scene_name", obs_source_get_name(obs_scene_get_source(scene)));
	obs_data_set_string(temp, "scene_uuid", obs_source_get_uuid(obs_scene_get_source(scene)));
	obs_data_set_bool(temp, "is_group", false);

	obs_scene_enum_items(scene, save_transform_states, (void *)&pass);

	obs_data_set_array(temp, "items", item_ids);
	obs_data_array_push_back(scenes_and_groups, temp);

	obs_data_set_array(wrapper, "scenes_and_groups", scenes_and_groups);

	obs_data_array_release(item_ids);
	obs_data_array_release(scenes_and_groups);
	obs_data_release(temp);

	return wrapper;
}

void load_transform_states(obs_data_t *temp, void *vp_scene)
{
	obs_scene_t *scene = (obs_scene_t *)vp_scene;
	int64_t id = obs_data_get_int(temp, "id");
	obs_sceneitem_t *item = obs_scene_find_sceneitem_by_id(scene, id);

	struct obs_transform_info info;
	struct obs_sceneitem_crop crop;
	obs_data_get_vec2(temp, "pos", &info.pos);
	obs_data_get_vec2(temp, "scale", &info.scale);
	info.rot = (float)obs_data_get_double(temp, "rot");
	info.alignment = (uint32_t)obs_data_get_int(temp, "alignment");
	info.bounds_type = valid_bounds_type(obs_data_get_int(temp, "bounds_type"));
	info.bounds_alignment = (uint32_t)obs_data_get_int(temp, "bounds_alignment");
	obs_data_get_vec2(temp, "bounds", &info.bounds);
	info.crop_to_bounds = obs_data_get_bool(temp, "crop_to_bounds");
	crop.top = (int)obs_data_get_int(temp, "top");
	crop.bottom = (int)obs_data_get_int(temp, "bottom");
	crop.left = (int)obs_data_get_int(temp, "left");
	crop.right = (int)obs_data_get_int(temp, "right");

	obs_sceneitem_defer_update_begin(item);

	obs_sceneitem_set_info2(item, &info);
	obs_sceneitem_set_crop(item, &crop);

	obs_sceneitem_defer_update_end(item);
}

void iterate_scenes_and_groups_transform_states(obs_data_t *data, void *vp)
{
	obs_data_array_t *items = obs_data_get_array(data, "items");
	obs_source_t *scene_source = obs_get_source_by_uuid(obs_data_get_string(data, "scene_uuid"));
	obs_scene_t *scene = obs_scene_from_source(scene_source);

	if (obs_data_get_bool(data, "is_group")) {
		obs_source_t *parent_source = obs_get_source_by_uuid(obs_data_get_string(data, "group_parent"));
		obs_scene_t *parent = obs_scene_from_source(parent_source);
		obs_sceneitem_t *group = obs_scene_get_group(parent, obs_data_get_string(data, "scene_name"));
		scene = obs_sceneitem_group_get_scene(group);

		obs_source_release(parent_source);
	}

	obs_data_array_enum(items, load_transform_states, (void *)scene);

	UNUSED_PARAMETER(vp);

	obs_data_array_release(items);
	obs_source_release(scene_source);
}

void obs_scene_load_transform_states(const char *data)
{
	obs_data_t *dat = obs_data_create_from_json(data);

	obs_data_array_t *scenes_and_groups = obs_data_get_array(dat, "scenes_and_groups");

	obs_data_array_enum(scenes_and_groups, iterate_scenes_and_groups_transform_states, NULL);

	obs_data_release(dat);
	obs_data_array_release(scenes_and_groups);
}

void obs_sceneitem_select(obs_sceneitem_t *item, bool select)
{
	struct calldata params;
	uint8_t stack[128];
	const char *command = select ? "item_select" : "item_deselect";

	if (!item || item->selected == select || !item->parent)
		return;

	item->selected = select;

	calldata_init_fixed(&params, stack, sizeof(stack));
	calldata_set_ptr(&params, "item", item);

	signal_parent(item->parent, command, &params);
}

bool obs_sceneitem_selected(const obs_sceneitem_t *item)
{
	return item ? item->selected : false;
}

#define do_update_transform(item)                                          \
	do {                                                               \
		if (!item->parent || item->parent->is_group)               \
			os_atomic_set_bool(&item->update_transform, true); \
		else                                                       \
			update_item_transform(item, false);                \
	} while (false)

void obs_sceneitem_set_pos(obs_sceneitem_t *item, const struct vec2 *pos)
{
	if (item) {
		if (!item->absolute_coordinates)
			pos_from_absolute(&item->pos, pos, item);
		else
			vec2_copy(&item->pos, pos);

		do_update_transform(item);
	}
}

void obs_sceneitem_set_rot(obs_sceneitem_t *item, float rot)
{
	if (item) {
		item->rot = rot;
		do_update_transform(item);
	}
}

void obs_sceneitem_set_scale(obs_sceneitem_t *item, const struct vec2 *scale)
{
	if (item) {
		if (!item->absolute_coordinates)
			item_relative_scale(&item->scale, scale, item);
		else
			vec2_copy(&item->scale, scale);

		do_update_transform(item);
	}
}

void obs_sceneitem_set_alignment(obs_sceneitem_t *item, uint32_t alignment)
{
	if (item) {
		item->align = alignment;
		do_update_transform(item);
	}
}

static inline void signal_reorder(struct obs_scene_item *item)
{
	const char *command = NULL;
	struct calldata params;
	uint8_t stack[128];

	command = "reorder";

	calldata_init_fixed(&params, stack, sizeof(stack));
	signal_parent(item->parent, command, &params);
}

static inline void signal_refresh(obs_scene_t *scene)
{
	const char *command = NULL;
	struct calldata params;
	uint8_t stack[128];

	command = "refresh";

	calldata_init_fixed(&params, stack, sizeof(stack));
	signal_parent(scene, command, &params);
}

void obs_sceneitem_set_order(obs_sceneitem_t *item, enum obs_order_movement movement)
{
	if (!item)
		return;

	struct obs_scene_item *next, *prev;
	struct obs_scene *scene = obs_scene_get_ref(item->parent);

	if (!scene)
		return;

	full_lock(scene);

	next = item->next;
	prev = item->prev;

	detach_sceneitem(item);

	if (movement == OBS_ORDER_MOVE_DOWN) {
		attach_sceneitem(scene, item, prev ? prev->prev : NULL);

	} else if (movement == OBS_ORDER_MOVE_UP) {
		attach_sceneitem(scene, item, next ? next : prev);

	} else if (movement == OBS_ORDER_MOVE_TOP) {
		struct obs_scene_item *last = next;
		if (!last) {
			last = prev;
		} else {
			while (last->next)
				last = last->next;
		}

		attach_sceneitem(scene, item, last);

	} else if (movement == OBS_ORDER_MOVE_BOTTOM) {
		attach_sceneitem(scene, item, NULL);
	}

	full_unlock(scene);

	signal_reorder(item);
	obs_scene_release(scene);
}

int obs_sceneitem_get_order_position(obs_sceneitem_t *item)
{
	struct obs_scene *scene = item->parent;
	struct obs_scene_item *next = scene->first_item;

	full_lock(scene);

	int index = 0;
	while (next && next != item) {
		next = next->next;
		++index;
	}

	full_unlock(scene);

	return index;
}

void obs_sceneitem_set_order_position(obs_sceneitem_t *item, int position)
{
	if (!item)
		return;

	struct obs_scene *scene = obs_scene_get_ref(item->parent);

	if (!scene)
		return;

	full_lock(scene);

	detach_sceneitem(item);

	if (!scene->first_item || position == 0) {
		attach_sceneitem(scene, item, NULL);
	} else {
		struct obs_scene_item *next = scene->first_item;

		for (int i = position; i > 1; --i) {
			if (next->next == NULL)
				break;
			next = next->next;
		}

		attach_sceneitem(scene, item, next);
	}

	full_unlock(scene);

	signal_reorder(item);
	obs_scene_release(scene);
}

void obs_sceneitem_set_bounds_type(obs_sceneitem_t *item, enum obs_bounds_type type)
{
	if (item) {
		item->bounds_type = valid_bounds_type(type);
		do_update_transform(item);
	}
}

void obs_sceneitem_set_bounds_alignment(obs_sceneitem_t *item, uint32_t alignment)
{
	if (item) {
		item->bounds_align = alignment;
		do_update_transform(item);
	}
}

void obs_sceneitem_set_bounds_crop(obs_sceneitem_t *item, bool crop)
{
	if (item) {
		item->crop_to_bounds = crop;
		do_update_transform(item);
	}
}

void obs_sceneitem_set_bounds(obs_sceneitem_t *item, const struct vec2 *bounds)
{
	if (item) {
		if (!item->absolute_coordinates)
			size_from_absolute(&item->bounds, bounds, item);
		else
			vec2_copy(&item->bounds, bounds);

		do_update_transform(item);
	}
}

void obs_sceneitem_get_pos(const obs_sceneitem_t *item, struct vec2 *pos)
{
	if (!item)
		return;

	if (!item->absolute_coordinates)
		pos_to_absolute(pos, &item->pos, item);
	else
		vec2_copy(pos, &item->pos);
}

float obs_sceneitem_get_rot(const obs_sceneitem_t *item)
{
	return item ? item->rot : 0.0f;
}

void obs_sceneitem_get_scale(const obs_sceneitem_t *item, struct vec2 *scale)
{
	if (!item)
		return;

	if (!item->absolute_coordinates)
		item_canvas_scale(scale, item);
	else
		vec2_copy(scale, &item->scale);
}

uint32_t obs_sceneitem_get_alignment(const obs_sceneitem_t *item)
{
	return item ? item->align : 0;
}

enum obs_bounds_type obs_sceneitem_get_bounds_type(const obs_sceneitem_t *item)
{
	return item ? item->bounds_type : OBS_BOUNDS_NONE;
}

uint32_t obs_sceneitem_get_bounds_alignment(const obs_sceneitem_t *item)
{
	return item ? item->bounds_align : 0;
}

bool obs_sceneitem_get_bounds_crop(const obs_sceneitem_t *item)
{
	return item ? item->crop_to_bounds : false;
}

void obs_sceneitem_get_bounds(const obs_sceneitem_t *item, struct vec2 *bounds)
{
	if (!item)
		return;

	if (!item->absolute_coordinates)
		size_to_absolute(bounds, &item->bounds, item);
	else
		vec2_copy(bounds, &item->bounds);
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
		scene_item_get_info_internal(item, info);
		info->crop_to_bounds = item->crop_to_bounds;
	}
}

static inline void scene_item_set_info_internal(obs_sceneitem_t *item, const struct obs_transform_info *info)
{
	if (!item->absolute_coordinates) {
		pos_from_absolute(&item->pos, &info->pos, item);
		size_from_absolute(&item->bounds, &info->bounds, item);
		if (isfinite(info->scale.x) && isfinite(info->scale.y)) {
			item_relative_scale(&item->scale, &info->scale, item);
		}
	} else {
		item->pos = info->pos;
		item->bounds = info->bounds;
		if (isfinite(info->scale.x) && isfinite(info->scale.y)) {
			item->scale = info->scale;
		}
	}

	item->rot = info->rot;
	item->align = info->alignment;
	item->bounds_type = valid_bounds_type(info->bounds_type);
	item->bounds_align = info->bounds_alignment;
}

void obs_sceneitem_set_info2(obs_sceneitem_t *item, const struct obs_transform_info *info)
{
	if (item && info) {
		scene_item_set_info_internal(item, info);
		item->crop_to_bounds = info->crop_to_bounds;
		do_update_transform(item);
	}
}

void obs_sceneitem_get_draw_transform(const obs_sceneitem_t *item, struct matrix4 *transform)
{
	if (item)
		matrix4_copy(transform, &item->draw_transform);
}

void obs_sceneitem_get_box_transform(const obs_sceneitem_t *item, struct matrix4 *transform)
{
	if (item)
		matrix4_copy(transform, &item->box_transform);
}

void obs_sceneitem_get_box_scale(const obs_sceneitem_t *item, struct vec2 *scale)
{
	if (item)
		*scale = item->box_scale;
}

bool obs_sceneitem_visible(const obs_sceneitem_t *item)
{
	return item ? item->user_visible : false;
}

static bool group_item_transition(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	if (!param || !item)
		return true;
	const bool visible = *(bool *)param;
	if (obs_sceneitem_visible(item))
		obs_sceneitem_do_transition(item, visible);
	UNUSED_PARAMETER(scene);
	return true;
}

bool obs_sceneitem_set_visible(obs_sceneitem_t *item, bool visible)
{
	struct calldata cd;
	uint8_t stack[256];
	struct item_action action = {.visible = visible, .timestamp = os_gettime_ns()};

	if (!item)
		return false;

	if (item->user_visible == visible)
		return false;

	if (!item->parent)
		return false;

	obs_sceneitem_do_transition(item, visible);
	if (obs_sceneitem_is_group(item))
		obs_sceneitem_group_enum_items(item, group_item_transition, &visible);

	item->user_visible = visible;

	if (visible) {
		if (os_atomic_inc_long(&item->active_refs) == 1) {
			if (!obs_source_add_active_child(item->parent->source, item->source)) {
				os_atomic_dec_long(&item->active_refs);
				return false;
			}
		}
	}

	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "item", item);
	calldata_set_bool(&cd, "visible", visible);

	signal_parent(item->parent, "item_visible", &cd);

	if (source_has_audio(item->source)) {
		pthread_mutex_lock(&item->actions_mutex);
		da_push_back(item->audio_actions, &action);
		pthread_mutex_unlock(&item->actions_mutex);
	} else {
		set_visibility(item, visible);
	}
	return true;
}

bool obs_sceneitem_locked(const obs_sceneitem_t *item)
{
	return item ? item->locked : false;
}

bool obs_sceneitem_set_locked(obs_sceneitem_t *item, bool lock)
{
	struct calldata cd;
	uint8_t stack[256];

	if (!item)
		return false;

	if (item->locked == lock)
		return false;

	if (!item->parent)
		return false;

	item->locked = lock;

	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "item", item);
	calldata_set_bool(&cd, "locked", lock);

	signal_parent(item->parent, "item_locked", &cd);

	return true;
}

static bool sceneitems_match(obs_scene_t *scene, obs_sceneitem_t *const *items, size_t size, bool *order_matches)
{
	obs_sceneitem_t *item = scene->first_item;

	size_t count = 0;
	while (item) {
		bool found = false;
		for (size_t i = 0; i < size; i++) {
			if (items[i] != item)
				continue;

			if (count != i)
				*order_matches = false;

			found = true;
			break;
		}

		if (!found)
			return false;

		item = item->next;
		count += 1;
	}

	return count == size;
}

bool obs_scene_reorder_items(obs_scene_t *scene, obs_sceneitem_t *const *item_order, size_t item_order_size)
{
	if (!scene || !item_order_size)
		return false;

	scene = obs_scene_get_ref(scene);
	if (!scene)
		return false;

	full_lock(scene);

	bool order_matches = true;
	if (!sceneitems_match(scene, item_order, item_order_size, &order_matches) || order_matches) {
		full_unlock(scene);
		obs_scene_release(scene);
		return false;
	}

	scene->first_item = item_order[0];

	obs_sceneitem_t *prev = NULL;
	for (size_t i = 0; i < item_order_size; i++) {
		item_order[i]->prev = prev;
		item_order[i]->next = NULL;

		if (prev)
			prev->next = item_order[i];

		prev = item_order[i];
	}

	full_unlock(scene);

	signal_reorder(scene->first_item);
	obs_scene_release(scene);
	return true;
}

void obs_scene_atomic_update(obs_scene_t *scene, obs_scene_atomic_update_func func, void *data)
{
	if (!scene)
		return;

	scene = obs_scene_get_ref(scene);
	if (!scene)
		return;

	full_lock(scene);
	func(data, scene);
	full_unlock(scene);
	obs_scene_release(scene);
}

static inline bool crop_equal(const struct obs_sceneitem_crop *crop1, const struct obs_sceneitem_crop *crop2)
{
	return crop1->left == crop2->left && crop1->right == crop2->right && crop1->top == crop2->top &&
	       crop1->bottom == crop2->bottom;
}

void obs_sceneitem_set_crop(obs_sceneitem_t *item, const struct obs_sceneitem_crop *crop)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_set_crop"))
		return;
	if (!obs_ptr_valid(crop, "obs_sceneitem_set_crop"))
		return;
	if (crop_equal(crop, &item->crop))
		return;

	memcpy(&item->crop, crop, sizeof(*crop));

	item->crop.left = obs_scene_clamp_crop(item->crop.left);
	item->crop.right = obs_scene_clamp_crop(item->crop.right);
	item->crop.top = obs_scene_clamp_crop(item->crop.top);
	item->crop.bottom = obs_scene_clamp_crop(item->crop.bottom);

	os_atomic_set_bool(&item->update_transform, true);
}

void obs_sceneitem_get_crop(const obs_sceneitem_t *item, struct obs_sceneitem_crop *crop)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_get_crop"))
		return;
	if (!obs_ptr_valid(crop, "obs_sceneitem_get_crop"))
		return;

	memcpy(crop, &item->crop, sizeof(*crop));
}

void obs_sceneitem_set_scale_filter(obs_sceneitem_t *item, enum obs_scale_type filter)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_set_scale_filter"))
		return;

	item->scale_filter = filter;

	os_atomic_set_bool(&item->update_transform, true);
}

enum obs_scale_type obs_sceneitem_get_scale_filter(obs_sceneitem_t *item)
{
	return obs_ptr_valid(item, "obs_sceneitem_get_scale_filter") ? item->scale_filter : OBS_SCALE_DISABLE;
}

void obs_sceneitem_set_blending_method(obs_sceneitem_t *item, enum obs_blending_method method)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_set_blending_method"))
		return;

	item->blend_method = method;
}

enum obs_blending_method obs_sceneitem_get_blending_method(obs_sceneitem_t *item)
{
	return obs_ptr_valid(item, "obs_sceneitem_get_blending_method") ? item->blend_method : OBS_BLEND_METHOD_DEFAULT;
}

void obs_sceneitem_set_blending_mode(obs_sceneitem_t *item, enum obs_blending_type type)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_set_blending_mode"))
		return;

	item->blend_type = type;

	os_atomic_set_bool(&item->update_transform, true);
}

enum obs_blending_type obs_sceneitem_get_blending_mode(obs_sceneitem_t *item)
{
	return obs_ptr_valid(item, "obs_sceneitem_get_blending_mode") ? item->blend_type : OBS_BLEND_NORMAL;
}

void obs_sceneitem_defer_update_begin(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_defer_update_begin"))
		return;

	os_atomic_inc_long(&item->defer_update);
}

void obs_sceneitem_defer_update_end(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_defer_update_end"))
		return;

	if (os_atomic_dec_long(&item->defer_update) == 0)
		do_update_transform(item);
}

void obs_sceneitem_defer_group_resize_begin(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_defer_group_resize_begin"))
		return;

	os_atomic_inc_long(&item->defer_group_resize);
}

void obs_sceneitem_defer_group_resize_end(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_defer_group_resize_end"))
		return;

	if (os_atomic_dec_long(&item->defer_group_resize) == 0)
		os_atomic_set_bool(&item->update_group_resize, true);
}

int64_t obs_sceneitem_get_id(const obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_get_id"))
		return 0;

	return item->id;
}

void obs_sceneitem_set_id(obs_sceneitem_t *item, int64_t id)
{
	item->id = id;
}

obs_data_t *obs_sceneitem_get_private_settings(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_get_private_settings"))
		return NULL;

	obs_data_addref(item->private_settings);
	return item->private_settings;
}

/* The matrix math is obs-scene-group-transform.c.h. The scene still reads
 * the item, still converts absolute and relative coordinates, and still
 * decides whether to invert the group's draw transform. */

static void get_ungrouped_transform(obs_sceneitem_t *group, obs_sceneitem_t *item, struct vec2 *pos, struct vec2 *scale,
				    float *rot)
{
	struct vec2 scale_abs;
	struct vec2 pos_abs;

	if (item->absolute_coordinates) {
		vec2_copy(&scale_abs, scale);
		vec2_copy(&pos_abs, pos);
	} else {
		size_to_absolute(&scale_abs, scale, item);
		pos_to_absolute(&pos_abs, pos, item);
	}

	obs_scene_group_transform(&group->draw_transform, &pos_abs, &scale_abs, rot, group->rot);

	if (item->absolute_coordinates) {
		vec2_copy(scale, &scale_abs);
		vec2_copy(pos, &pos_abs);
	} else {
		size_from_absolute(scale, &scale_abs, item);
		pos_from_absolute(pos, &pos_abs, item);
	}
}

/* A visible item's source is an active child of the scene that holds it.
 * When an item moves to another scene (into or out of a group), move that
 * relationship along, or activation and deactivation walk different trees and
 * the source's activate/show counts drift (obsproject/obs-studio#9961).
 * Add before removing so a source that stays on screen never drops to zero. */
struct contains_source_info {
	obs_source_t *target;
	bool found;
};

static void contains_source_cb(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct contains_source_info *info = param;
	UNUSED_PARAMETER(parent);
	if (child == info->target)
		info->found = true;
}

/* Whether moving `item` into `to` would make a scene contain itself: `to` is
 * the item's own source or somewhere inside it (obsproject/obs-studio#10466). */
static bool move_would_recurse(obs_scene_t *to, obs_sceneitem_t *item)
{
	if (item->source == to->source)
		return true;

	struct contains_source_info info = {to->source, false};
	obs_source_enum_full_tree(item->source, contains_source_cb, &info);
	return info.found;
}

static void move_active_child(obs_scene_t *from, obs_scene_t *to, obs_sceneitem_t *item)
{
	if (from == to || os_atomic_load_long(&item->active_refs) <= 0)
		return;

	if (to)
		obs_source_add_active_child(to->source, item->source);
	if (from)
		obs_source_remove_active_child(from->source, item->source);
}

static void remove_group_transform(obs_sceneitem_t *group, obs_sceneitem_t *item)
{
	obs_scene_t *parent = item->parent;
	if (!parent || !group)
		return;

	get_ungrouped_transform(group, item, &item->pos, &item->scale, &item->rot);

	update_item_transform(item, false);
}

static void apply_group_transform(obs_sceneitem_t *item, obs_sceneitem_t *group)
{
	struct matrix4 transform;
	struct vec2 scale_abs;
	struct vec2 pos_abs;
	float rot;

	if (item->absolute_coordinates) {
		vec2_copy(&scale_abs, &item->scale);
		vec2_copy(&pos_abs, &item->pos);
	} else {
		size_to_absolute(&scale_abs, &item->scale, item);
		pos_to_absolute(&pos_abs, &item->pos, item);
	}

	matrix4_inv(&transform, &group->draw_transform);

	rot = item->rot;
	obs_scene_group_transform(&transform, &pos_abs, &scale_abs, &rot, -group->rot);

	if (item->absolute_coordinates) {
		vec2_copy(&item->scale, &scale_abs);
		vec2_copy(&item->pos, &pos_abs);
	} else {
		size_from_absolute(&item->scale, &scale_abs, item);
		pos_from_absolute(&item->pos, &pos_abs, item);
	}
	item->rot = rot;

	update_item_transform(item, false);
}

/* The fold and the size are obs-scene-extent.c.h. The scene still walks
 * its items and still shifts the children so the content starts at the
 * origin. */

static bool resize_scene_base(obs_scene_t *scene, struct vec2 *minv, struct vec2 *maxv, struct vec2 *scale)
{
	obs_scene_extent_init(minv, maxv);

	obs_sceneitem_t *item = scene->first_item;
	if (!item) {
		scene->cx = 0;
		scene->cy = 0;
		return false;
	}

	while (item) {
		obs_scene_extent_add(minv, maxv, &item->box_transform);
		item = item->next;
	}

	item = scene->first_item;
	if (item) {
		struct vec2 minv_rel;
		if (!item->absolute_coordinates)
			size_from_absolute(&minv_rel, minv, item);
		else
			vec2_copy(&minv_rel, minv);

		while (item) {
			vec2_sub(&item->pos, &item->pos, &minv_rel);
			update_item_transform(item, false);
			item = item->next;
		}
	}

	obs_scene_extent_size(scale, &scene->cx, &scene->cy, minv, maxv);
	return true;
}

static void resize_scene(obs_scene_t *scene)
{
	struct vec2 minv;
	struct vec2 maxv;
	struct vec2 scale;
	resize_scene_base(scene, &minv, &maxv, &scale);
}

/* assumes group scene and parent scene is locked */
static void resize_group(obs_sceneitem_t *group, bool scene_resize)
{
	obs_scene_t *scene = group->source->context.data;
	struct vec2 minv;
	struct vec2 maxv;
	struct vec2 scale;

	if (os_atomic_load_long(&group->defer_group_resize) > 0)
		return;

	if (!resize_scene_base(scene, &minv, &maxv, &scale))
		return;

	if (group->bounds_type == OBS_BOUNDS_NONE && !scene_resize) {
		/* The children were shifted so the content starts at the group's
		 * origin; move the group so the content stays where it was on
		 * screen. The new position is measured with the new transform
		 * rather than derived from the content size, because the group
		 * draws with its size rounded (ceil, scale, integer halves for
		 * centred alignment) and any difference would add up with every
		 * resize (obsproject/obs-studio#9298). */
		struct vec2 target;
		struct vec2 now;
		struct vec2 delta;

		vec2_copy(&target, &minv);
		obs_scene_transform_val(&target, &group->draw_transform);

		update_item_transform(group, false);

		vec2_zero(&now);
		obs_scene_transform_val(&now, &group->draw_transform);

		vec2_sub(&delta, &target, &now);
		if (!group->absolute_coordinates)
			size_from_absolute(&delta, &delta, group);
		vec2_add(&group->pos, &group->pos, &delta);
	}

	os_atomic_set_bool(&group->update_group_resize, false);

	update_item_transform(group, false);
}

obs_sceneitem_t *obs_scene_add_group(obs_scene_t *scene, const char *name)
{
	return obs_scene_insert_group(scene, name, NULL, 0);
}

obs_sceneitem_t *obs_scene_add_group2(obs_scene_t *scene, const char *name, bool signal)
{
	return obs_scene_insert_group2(scene, name, NULL, 0, signal);
}

obs_sceneitem_t *obs_scene_insert_group(obs_scene_t *scene, const char *name, obs_sceneitem_t **items, size_t count)
{
	if (!scene)
		return NULL;

	/* don't allow groups or sub-items of other groups */
	for (size_t i = count; i > 0; i--) {
		obs_sceneitem_t *item = items[i - 1];
		if (item->parent != scene || item->is_group)
			return NULL;
	}

	obs_canvas_t *canvas = obs_weak_canvas_get_canvas(scene->source->canvas);
	obs_scene_t *sub_scene = create_id(canvas, group_info.id, name);
	obs_canvas_release(canvas);

	obs_sceneitem_t *last_item = items ? items[count - 1] : NULL;
	obs_sceneitem_t *item = obs_scene_add_internal(scene, sub_scene->source, last_item, 0);

	if (!items || !count) {
		obs_scene_release(sub_scene);
		return item;
	}

	/* ------------------------- */

	full_lock(scene);
	full_lock(sub_scene);
	sub_scene->first_item = items[0];

	for (size_t i = count; i > 0; i--) {
		size_t idx = i - 1;
		remove_group_transform(item, items[idx]);
		detach_sceneitem(items[idx]);
	}
	for (size_t i = 0; i < count; i++) {
		size_t idx = i;
		if (idx != (count - 1)) {
			size_t next_idx = idx + 1;
			items[idx]->next = items[next_idx];
			items[next_idx]->prev = items[idx];
		} else {
			items[idx]->next = NULL;
		}
		items[idx]->parent = sub_scene;
		move_active_child(scene, sub_scene, items[idx]);
		apply_group_transform(items[idx], item);
	}
	items[0]->prev = NULL;
	resize_group(item, false);
	full_unlock(sub_scene);
	full_unlock(scene);

	struct calldata params;
	uint8_t stack[128];

	calldata_init_fixed(&params, stack, sizeof(stack));
	calldata_set_ptr(&params, "scene", scene);
	calldata_set_ptr(&params, "item", item);
	signal_handler_signal(scene->source->context.signals, "item_add", &params);

	/* ------------------------- */

	obs_scene_release(sub_scene);
	return item;
}

obs_sceneitem_t *obs_scene_insert_group2(obs_scene_t *scene, const char *name, obs_sceneitem_t **items, size_t count,
					 bool signal)
{
	obs_sceneitem_t *item = obs_scene_insert_group(scene, name, items, count);
	if (signal && item)
		signal_refresh(scene);
	return item;
}

obs_sceneitem_t *obs_scene_get_group(obs_scene_t *scene, const char *name)
{
	if (!scene || !name || !*name) {
		return NULL;
	}

	obs_sceneitem_t *group = NULL;
	obs_sceneitem_t *item;

	full_lock(scene);

	item = scene->first_item;
	while (item) {
		if (item->is_group && item->source->context.name) {
			if (strcmp(item->source->context.name, name) == 0) {
				group = item;
				break;
			}
		}

		item = item->next;
	}

	full_unlock(scene);

	return group;
}

bool obs_sceneitem_is_group(obs_sceneitem_t *item)
{
	return item && item->is_group;
}

obs_scene_t *obs_sceneitem_group_get_scene(const obs_sceneitem_t *item)
{
	return (item && item->is_group) ? item->source->context.data : NULL;
}

void obs_sceneitem_group_ungroup(obs_sceneitem_t *item)
{
	if (!item || !item->is_group)
		return;

	obs_scene_t *scene = item->parent;
	obs_scene_t *subscene = item->source->context.data;
	obs_sceneitem_t *insert_after = item;
	obs_sceneitem_t *first;
	obs_sceneitem_t *last;

	signal_item_remove(scene, item);

	full_lock(scene);

	/* ------------------------- */

	full_lock(subscene);
	first = subscene->first_item;
	last = first;
	while (last) {
		obs_sceneitem_t *dst;

		remove_group_transform(item, last);
		dst = obs_scene_add_internal(scene, last->source, insert_after, 0);
		duplicate_item_data(dst, last, true, true);
		apply_group_transform(last, item);

		if (!last->next)
			break;

		insert_after = dst;
		last = last->next;
	}
	full_unlock(subscene);

	/* ------------------------- */

	detach_sceneitem(item);
	full_unlock(scene);

	obs_sceneitem_release(item);
}

void obs_sceneitem_group_ungroup2(obs_sceneitem_t *item, bool signal)
{
	obs_scene_t *scene = item->parent;
	obs_sceneitem_group_ungroup(item);
	if (signal)
		signal_refresh(scene);
}

void obs_sceneitem_group_add_item(obs_sceneitem_t *group, obs_sceneitem_t *item)
{
	if (!group || !group->is_group || !item)
		return;

	obs_scene_t *scene = group->parent;
	obs_scene_t *groupscene = group->source->context.data;

	if (item->parent != scene)
		return;

	if (item->parent == groupscene)
		return;

	if (move_would_recurse(groupscene, item)) {
		blog(LOG_WARNING, "Not adding '%s' to group '%s': it contains the group",
		     obs_source_get_name(item->source), obs_source_get_name(group->source));
		return;
	}

	/* ------------------------- */

	full_lock(scene);
	full_lock(groupscene);

	remove_group_transform(group, item);

	detach_sceneitem(item);
	attach_sceneitem(groupscene, item, NULL);
	move_active_child(scene, groupscene, item);

	apply_group_transform(item, group);

	resize_group(group, false);

	full_unlock(groupscene);
	full_unlock(scene);

	/* ------------------------- */

	signal_refresh(scene);
}

void obs_sceneitem_group_remove_item(obs_sceneitem_t *group, obs_sceneitem_t *item)
{
	if (!item || !group || !group->is_group)
		return;

	obs_scene_t *groupscene = item->parent;
	obs_scene_t *scene = group->parent;

	/* ------------------------- */

	full_lock(scene);
	full_lock(groupscene);

	remove_group_transform(group, item);

	detach_sceneitem(item);
	attach_sceneitem(scene, item, NULL);
	move_active_child(groupscene, scene, item);

	resize_group(group, false);

	full_unlock(groupscene);
	full_unlock(scene);

	/* ------------------------- */

	signal_refresh(scene);
}

static void build_current_order_info(obs_scene_t *scene, struct obs_sceneitem_order_info **items_out, size_t *size_out)
{
	DARRAY(struct obs_sceneitem_order_info) items;
	da_init(items);

	obs_sceneitem_t *item = scene->first_item;
	while (item) {
		struct obs_sceneitem_order_info info = {0};
		info.item = item;
		da_push_back(items, &info);

		if (item->is_group) {
			obs_scene_t *sub_scene = item->source->context.data;

			full_lock(sub_scene);

			obs_sceneitem_t *sub_item = sub_scene->first_item;

			while (sub_item) {
				info.group = item;
				info.item = sub_item;
				da_push_back(items, &info);

				sub_item = sub_item->next;
			}

			full_unlock(sub_scene);
		}

		item = item->next;
	}

	*items_out = items.array;
	*size_out = items.num;
}

static bool sceneitems_match2(obs_scene_t *scene, struct obs_sceneitem_order_info *items, size_t size)
{
	struct obs_sceneitem_order_info *cur_items;
	size_t cur_size;

	build_current_order_info(scene, &cur_items, &cur_size);
	if (cur_size != size) {
		bfree(cur_items);
		return false;
	}

	for (size_t i = 0; i < size; i++) {
		struct obs_sceneitem_order_info *new = &items[i];
		struct obs_sceneitem_order_info *old = &cur_items[i];

		if (new->group != old->group || new->item != old->item) {
			bfree(cur_items);
			return false;
		}
	}

	bfree(cur_items);
	return true;
}

static obs_sceneitem_t *get_sceneitem_parent_group(obs_scene_t *scene, obs_sceneitem_t *group_subitem)
{
	if (group_subitem->is_group)
		return NULL;

	obs_sceneitem_t *item = scene->first_item;
	while (item) {
		if (item->is_group && item->source->context.data == group_subitem->parent)
			return item;
		item = item->next;
	}

	return NULL;
}

static void obs_sceneitem_move_hotkeys(obs_scene_t *parent, obs_sceneitem_t *item)
{
	obs_data_array_t *data0 = NULL;
	obs_data_array_t *data1 = NULL;

	obs_hotkey_pair_save(item->toggle_visibility, &data0, &data1);
	obs_hotkey_pair_unregister(item->toggle_visibility);

	init_hotkeys(parent, item, obs_source_get_name(item->source));
	obs_hotkey_pair_load(item->toggle_visibility, data0, data1);

	obs_data_array_release(data0);
	obs_data_array_release(data1);
}

bool obs_scene_reorder_items2(obs_scene_t *scene, struct obs_sceneitem_order_info *item_order, size_t item_order_size)
{
	if (!scene || !item_order_size || !item_order)
		return false;

	for (size_t i = 0; i < item_order_size; i++) {
		struct obs_sceneitem_order_info *info = &item_order[i];
		if (!info->group || !info->item || !info->group->is_group)
			continue;

		obs_scene_t *sub_scene = info->group->source->context.data;
		if (info->item->parent != sub_scene && move_would_recurse(sub_scene, info->item)) {
			blog(LOG_WARNING, "Not reordering: '%s' cannot go into group '%s', which it contains",
			     obs_source_get_name(info->item->source), obs_source_get_name(info->group->source));
			return false;
		}
	}

	scene = obs_scene_get_ref(scene);
	if (!scene)
		return false;

	full_lock(scene);

	if (sceneitems_match2(scene, item_order, item_order_size)) {
		full_unlock(scene);
		obs_scene_release(scene);
		return false;
	}

	for (size_t i = 0; i < item_order_size; i++) {
		struct obs_sceneitem_order_info *info = &item_order[i];
		if (!info->item->is_group) {
			obs_sceneitem_t *group = get_sceneitem_parent_group(scene, info->item);
			remove_group_transform(group, info->item);
		}
	}

	scene->first_item = item_order[0].item;

	obs_sceneitem_t *prev = NULL;
	for (size_t i = 0; i < item_order_size; i++) {
		struct obs_sceneitem_order_info *info = &item_order[i];
		obs_sceneitem_t *item = info->item;

		if (info->item->is_group) {
			obs_sceneitem_t *sub_prev = NULL;
			obs_scene_t *sub_scene = info->item->source->context.data;

			sub_scene->first_item = NULL;

			obs_scene_addref(sub_scene);
			full_lock(sub_scene);

			for (i++; i < item_order_size; i++) {
				struct obs_sceneitem_order_info *sub_info = &item_order[i];
				obs_sceneitem_t *sub_item = sub_info->item;

				if (sub_info->group != info->item) {
					i--;
					break;
				}

				if (!sub_scene->first_item)
					sub_scene->first_item = sub_item;

				/* Move hotkeys into group */
				obs_sceneitem_move_hotkeys(sub_scene, sub_item);

				sub_item->prev = sub_prev;
				sub_item->next = NULL;
				move_active_child(sub_item->parent, sub_scene, sub_item);
				sub_item->parent = sub_scene;

				if (sub_prev)
					sub_prev->next = sub_item;

				apply_group_transform(sub_info->item, sub_info->group);

				sub_prev = sub_item;
			}

			resize_group(info->item, false);
			full_unlock(sub_scene);
			obs_scene_release(sub_scene);
		}

		/* Move item hotkeys out of group */
		if (item->parent && obs_scene_is_group(item->parent))
			obs_sceneitem_move_hotkeys(scene, item);

		item->prev = prev;
		item->next = NULL;
		move_active_child(item->parent, scene, item);
		item->parent = scene;

		if (prev)
			prev->next = item;

		prev = item;
	}

	full_unlock(scene);

	signal_reorder(scene->first_item);
	obs_scene_release(scene);
	return true;
}

obs_sceneitem_t *obs_sceneitem_get_group(obs_scene_t *scene, obs_sceneitem_t *group_subitem)
{
	if (!scene || !group_subitem || group_subitem->is_group)
		return NULL;

	full_lock(scene);
	obs_sceneitem_t *group = get_sceneitem_parent_group(scene, group_subitem);
	full_unlock(scene);

	return group;
}

bool obs_source_is_group(const obs_source_t *source)
{
	return source && strcmp(source->info.id, group_info.id) == 0;
}

bool obs_source_type_is_group(const char *id)
{
	return id && strcmp(id, group_info.id) == 0;
}

bool obs_source_is_scene(const obs_source_t *source)
{
	return source && strcmp(source->info.id, scene_info.id) == 0;
}

bool obs_source_type_is_scene(const char *id)
{
	return id && strcmp(id, scene_info.id) == 0;
}

bool obs_scene_is_group(const obs_scene_t *scene)
{
	return scene ? scene->is_group : false;
}

void obs_sceneitem_group_enum_items(obs_sceneitem_t *group, bool (*callback)(obs_scene_t *, obs_sceneitem_t *, void *),
				    void *param)
{
	if (!group || !group->is_group)
		return;

	obs_scene_t *scene = group->source->context.data;
	if (scene)
		obs_scene_enum_items(scene, callback, param);
}

void obs_sceneitem_force_update_transform(obs_sceneitem_t *item)
{
	if (!item)
		return;

	if (os_atomic_set_bool(&item->update_transform, false))
		update_item_transform(item, false);
}

void obs_sceneitem_set_transition(obs_sceneitem_t *item, bool show, obs_source_t *transition)
{
	if (!item)
		return;

	obs_source_t **target = show ? &item->show_transition : &item->hide_transition;
	if (*target)
		obs_source_release(*target);
	*target = obs_source_get_ref(transition);
}

obs_source_t *obs_sceneitem_get_transition(obs_sceneitem_t *item, bool show)
{
	if (!item)
		return NULL;

	return show ? item->show_transition : item->hide_transition;
}

void obs_sceneitem_set_transition_duration(obs_sceneitem_t *item, bool show, uint32_t duration_ms)
{
	if (!item)
		return;
	if (show)
		item->show_transition_duration = duration_ms;
	else
		item->hide_transition_duration = duration_ms;
}

uint32_t obs_sceneitem_get_transition_duration(obs_sceneitem_t *item, bool show)
{
	if (!item)
		return 0;
	return show ? item->show_transition_duration : item->hide_transition_duration;
}

void obs_sceneitem_transition_stop(void *data, calldata_t *calldata)
{
	obs_source_t *parent = data;
	obs_source_t *transition;
	calldata_get_ptr(calldata, "source", &transition);
	obs_source_remove_active_child(parent, transition);
	signal_handler_t *sh = obs_source_get_signal_handler(transition);
	if (sh)
		signal_handler_disconnect(sh, "transition_stop", obs_sceneitem_transition_stop, parent);
}

/* Stop a running item transition for good. The transition_stop signal takes it
 * out of the scene's active children while it still holds its sources; the
 * transition must then stop running too, or the scene's active tree would
 * still list it and deactivate it a second time. */
static void stop_item_transition(obs_source_t *transition)
{
	if (!scene_transition_active(transition))
		return;
	obs_transition_force_stop(transition);
	obs_transition_clear(transition);
}

void obs_sceneitem_do_transition(obs_sceneitem_t *item, bool visible)
{
	if (!item)
		return;

	stop_item_transition(item->show_transition);
	stop_item_transition(item->hide_transition);

	obs_source_t *transition = obs_sceneitem_get_transition(item, visible);
	if (!transition)
		return;

	int duration = (int)obs_sceneitem_get_transition_duration(item, visible);

	const int cx = obs_source_get_width(item->source);
	const int cy = obs_source_get_height(item->source);
	obs_transition_set_size(transition, cx, cy);
	obs_transition_set_alignment(transition, OBS_ALIGN_CENTER);
	obs_transition_set_scale_type(transition, OBS_TRANSITION_SCALE_ASPECT);

	if (duration == 0)
		duration = 300;

	obs_scene_t *scene = obs_sceneitem_get_scene(item);
	obs_source_t *parent = obs_scene_get_source(scene);
	obs_source_add_active_child(parent, transition);

	signal_handler_t *sh = obs_source_get_signal_handler(transition);
	if (sh)
		signal_handler_connect(sh, "transition_stop", obs_sceneitem_transition_stop, parent);

	if (!visible) {
		obs_transition_set(transition, item->source);
		obs_transition_start(transition, OBS_TRANSITION_MODE_AUTO, duration, NULL);
	} else {
		obs_transition_set(transition, NULL);
		obs_transition_start(transition, OBS_TRANSITION_MODE_AUTO, duration, item->source);
	}
}

void obs_sceneitem_transition_load(struct obs_scene_item *item, obs_data_t *data, bool show)
{
	if (!item || !data)
		return;
	const char *id = obs_data_get_string(data, "id");
	if (id && strlen(id)) {
		const char *tn = obs_data_get_string(data, "name");
		obs_data_t *s = obs_data_get_obj(data, "transition");
		obs_source_t *t = obs_source_create_private(id, tn, s);
		obs_sceneitem_set_transition(item, show, t);
		obs_source_release(t);
		obs_data_release(s);
	} else {
		obs_sceneitem_set_transition(item, show, NULL);
	}
	obs_sceneitem_set_transition_duration(item, show, (uint32_t)obs_data_get_int(data, "duration"));
}

obs_data_t *obs_sceneitem_transition_save(struct obs_scene_item *item, bool show)
{
	obs_data_t *data = obs_data_create();
	struct obs_source *transition = show ? item->show_transition : item->hide_transition;
	if (transition) {
		obs_data_set_string(data, "id", obs_source_get_unversioned_id(transition));
		obs_data_set_string(data, "versioned_id", obs_source_get_id(transition));
		obs_data_set_string(data, "name", obs_source_get_name(transition));
		obs_data_t *s = obs_source_get_settings(transition);
		obs_data_set_obj(data, "transition", s);
		obs_data_release(s);
	}
	obs_data_set_int(data, "duration", show ? item->show_transition_duration : item->hide_transition_duration);
	return data;
}

void obs_scene_prune_sources(obs_scene_t *scene)
{
	obs_scene_item_ptr_array_t remove_items;
	da_init(remove_items);

	video_lock(scene);
	update_transforms_and_prune_sources(scene, &remove_items, NULL, false);
	video_unlock(scene);

	for (size_t i = 0; i < remove_items.num; i++)
		obs_sceneitem_release(remove_items.array[i]);

	da_free(remove_items);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef audio_lock
#undef video_lock
#undef audio_unlock
#undef video_unlock
#undef do_update_transform
