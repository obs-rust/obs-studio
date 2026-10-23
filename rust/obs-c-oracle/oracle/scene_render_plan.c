/* Test-only oracle: the original libobs/obs-scene-render-plan.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. */
#include <stddef.h>

#define obs_scene_render_texture_enabled oracle_obs_scene_render_texture_enabled
#define obs_scene_render_texrender_action oracle_obs_scene_render_texrender_action
#define obs_scene_render_size_empty oracle_obs_scene_render_size_empty
#define obs_scene_render_clear_white oracle_obs_scene_render_clear_white
#define obs_scene_render_child oracle_obs_scene_render_child
#define obs_scene_render_linear_srgb oracle_obs_scene_render_linear_srgb
#define obs_scene_texcoords_centered oracle_obs_scene_texcoords_centered
#define obs_scene_render_effect oracle_obs_scene_render_effect
#define obs_scene_render_multiplier oracle_obs_scene_render_multiplier
#define obs_scene_render_technique oracle_obs_scene_render_technique

/* The moved are_texcoords_centered() initializes struct matrix4's vec4
 * unions with flat lists, as libobs (-Wno-missing-braces) allows and the
 * oracle's -Wall does not. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wmissing-braces"
#endif
#include "obs-scene-render-plan.c.h"
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

#undef obs_scene_render_technique
#undef obs_scene_render_multiplier
#undef obs_scene_render_effect
#undef obs_scene_texcoords_centered
#undef obs_scene_render_linear_srgb
#undef obs_scene_render_child
#undef obs_scene_render_clear_white
#undef obs_scene_render_size_empty
#undef obs_scene_render_texrender_action
#undef obs_scene_render_texture_enabled

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/scene_render_plan.rs. */
enum {
	ORACLE_SCENE_RENDER_PLAN_LAYOUT_COUNT = 59,
};

size_t oracle_scene_render_plan_layout(size_t which);

size_t oracle_scene_render_plan_layout(size_t which)
{
	const size_t values[ORACLE_SCENE_RENDER_PLAN_LAYOUT_COUNT] = {
		/* struct obs_scene_render_triggers: 0-14 */
		sizeof(struct obs_scene_render_triggers),
		_Alignof(struct obs_scene_render_triggers),
		offsetof(struct obs_scene_render_triggers, crop_left),
		offsetof(struct obs_scene_render_triggers, crop_top),
		offsetof(struct obs_scene_render_triggers, crop_right),
		offsetof(struct obs_scene_render_triggers, crop_bottom),
		offsetof(struct obs_scene_render_triggers, bounds_crop_left),
		offsetof(struct obs_scene_render_triggers, bounds_crop_top),
		offsetof(struct obs_scene_render_triggers, bounds_crop_right),
		offsetof(struct obs_scene_render_triggers, bounds_crop_bottom),
		offsetof(struct obs_scene_render_triggers, scale_filter),
		offsetof(struct obs_scene_render_triggers, blend_method),
		offsetof(struct obs_scene_render_triggers, blend_type),
		offsetof(struct obs_scene_render_triggers, is_scene),
		offsetof(struct obs_scene_render_triggers, is_group),
		/* struct obs_scene_render_effect_plan: 15-20 */
		sizeof(struct obs_scene_render_effect_plan),
		_Alignof(struct obs_scene_render_effect_plan),
		offsetof(struct obs_scene_render_effect_plan, effect),
		offsetof(struct obs_scene_render_effect_plan, point_sampler),
		offsetof(struct obs_scene_render_effect_plan, base_dimension),
		offsetof(struct obs_scene_render_effect_plan, upscale),
		/* struct matrix4: 21-23 */
		sizeof(struct matrix4),
		_Alignof(struct matrix4),
		offsetof(struct matrix4, t),
		/* enum sizes: 24-27 */
		sizeof(enum obs_scene_render_texrender),
		sizeof(enum obs_scene_render_child_id),
		sizeof(enum obs_scene_render_effect_id),
		sizeof(enum obs_scene_render_technique_id),
		/* enum obs_scene_render_texrender: 28-32 */
		OBS_SCENE_RENDER_TEXRENDER_NONE,
		OBS_SCENE_RENDER_TEXRENDER_KEEP,
		OBS_SCENE_RENDER_TEXRENDER_DESTROY,
		OBS_SCENE_RENDER_TEXRENDER_CREATE,
		OBS_SCENE_RENDER_TEXRENDER_RECREATE,
		/* enum obs_scene_render_child_id: 33-35 */
		OBS_SCENE_RENDER_CHILD_SOURCE,
		OBS_SCENE_RENDER_CHILD_SHOW,
		OBS_SCENE_RENDER_CHILD_HIDE,
		/* enum obs_scene_render_effect_id: 36-40 */
		OBS_SCENE_RENDER_EFFECT_DEFAULT,
		OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES,
		OBS_SCENE_RENDER_EFFECT_BICUBIC,
		OBS_SCENE_RENDER_EFFECT_LANCZOS,
		OBS_SCENE_RENDER_EFFECT_AREA,
		/* enum obs_scene_render_technique_id: 41-48 */
		OBS_SCENE_RENDER_TECH_DRAW,
		OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY,
		OBS_SCENE_RENDER_TECH_DRAW_TONEMAP,
		OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP,
		OBS_SCENE_RENDER_TECH_DRAW_UPSCALE,
		OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY,
		OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP,
		OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP,
		/* the mirrored constants: 49-58 */
		OBS_SCENE_RENDER_SCALE_DISABLE,
		OBS_SCENE_RENDER_SCALE_POINT,
		OBS_SCENE_RENDER_SCALE_BICUBIC,
		OBS_SCENE_RENDER_SCALE_LANCZOS,
		OBS_SCENE_RENDER_SCALE_AREA,
		OBS_SCENE_RENDER_BLEND_METHOD_SRGB_OFF,
		OBS_SCENE_RENDER_BLEND_NORMAL,
		OBS_SCENE_RENDER_BLEND_DARKEN,
		OBS_SCENE_RENDER_CS_709_EXTENDED,
		OBS_SCENE_RENDER_CS_709_SCRGB,
	};
	return which < ORACLE_SCENE_RENDER_PLAN_LAYOUT_COUNT ? values[which] : 0;
}
