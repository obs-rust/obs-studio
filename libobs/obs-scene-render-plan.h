/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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

#pragma once

#include <stdbool.h>
#include <stdint.h>

#include "graphics/matrix4.h"

/* The per-item render decisions of render_item() and render_item_texture() in
 * obs-scene.c, on plain values (obs-scene-render-plan.c). Each one is made
 * where the C made it, from values the caller reads at that point:
 * - obs_scene_render_texture_enabled(): draw the item through its texrender
 *   (a crop, a bounds crop, a scale filter, sRGB off, a blend mode other
 *   than Normal, or a nested scene that is not a group);
 * - obs_scene_render_texrender_action(): keep, destroy, create or recreate
 *   the item's texrender;
 * - obs_scene_render_size_empty(): a source with no size (#573: the item's
 *   running show/hide transition is still rendered, at size 0);
 * - obs_scene_render_clear_white(): the texrender's clear colour (#1406:
 *   white for Darken);
 * - obs_scene_render_child(): what is drawn, the show transition, the hide
 *   transition or the source;
 * - obs_scene_render_linear_srgb(): the linear sRGB state the item is drawn
 *   with;
 * - obs_scene_texcoords_centered(): whether a directly drawn source gets
 *   centered texture coordinates (a byte compare, as before);
 * - obs_scene_render_effect(): the texture's effect from its scale filter
 *   and output scale, the point sampler, the base dimension parameters and
 *   Area's upscale;
 * - obs_scene_render_multiplier(): the SDR white multiplier between the
 *   source's and the target's colour space;
 * - obs_scene_render_technique(): the technique the texture is drawn with.
 *
 * obs-scene.c keeps every gs_* call, obs_source_video_render(), the
 * texrender objects and obs_get_video_sdr_white_level() (it passes the
 * value), and maps the effect and technique values to obs->video.* and to
 * the technique names. Nothing here takes a lock, calls into libobs or
 * keeps state.
 *
 * Enum values cross as int, so this header needs no core header;
 * obs-scene.c checks the constants below against obs.h and graphics.h.
 *
 * Hidden in libobs: not EXPORT. */

/* enum obs_scale_type */
#define OBS_SCENE_RENDER_SCALE_DISABLE 0
#define OBS_SCENE_RENDER_SCALE_POINT 1
#define OBS_SCENE_RENDER_SCALE_BICUBIC 2
#define OBS_SCENE_RENDER_SCALE_LANCZOS 4
#define OBS_SCENE_RENDER_SCALE_AREA 5
/* enum obs_blending_method */
#define OBS_SCENE_RENDER_BLEND_METHOD_SRGB_OFF 1
/* enum obs_blending_type */
#define OBS_SCENE_RENDER_BLEND_NORMAL 0
#define OBS_SCENE_RENDER_BLEND_DARKEN 6
/* enum gs_color_space */
#define OBS_SCENE_RENDER_CS_SRGB 0
#define OBS_SCENE_RENDER_CS_SRGB_16F 1
#define OBS_SCENE_RENDER_CS_709_EXTENDED 2
#define OBS_SCENE_RENDER_CS_709_SCRGB 3

/* The item's fields that decide whether it is drawn through a texture. */
struct obs_scene_render_triggers {
	/* item->crop */
	int crop_left;
	int crop_top;
	int crop_right;
	int crop_bottom;
	/* item->bounds_crop */
	int bounds_crop_left;
	int bounds_crop_top;
	int bounds_crop_right;
	int bounds_crop_bottom;
	/* item->scale_filter, an enum obs_scale_type */
	int scale_filter;
	/* item->blend_method, an enum obs_blending_method */
	int blend_method;
	/* item->blend_type, an enum obs_blending_type */
	int blend_type;
	/* item->source && item->source->info.type == OBS_SOURCE_TYPE_SCENE */
	bool is_scene;
	/* item->is_group */
	bool is_group;
};

bool obs_scene_render_texture_enabled(const struct obs_scene_render_triggers *triggers);

/* What render_item() does with item->item_render. */
enum obs_scene_render_texrender {
	/* none, and none is wanted */
	OBS_SCENE_RENDER_TEXRENDER_NONE,
	/* keep the one there */
	OBS_SCENE_RENDER_TEXRENDER_KEEP,
	/* destroy it */
	OBS_SCENE_RENDER_TEXRENDER_DESTROY,
	/* create one */
	OBS_SCENE_RENDER_TEXRENDER_CREATE,
	/* destroy it and create one in the source's format */
	OBS_SCENE_RENDER_TEXRENDER_RECREATE,
};

/* has_texrender: item->item_render != NULL. use_texrender:
 * obs_scene_render_texture_enabled(). format_matches: the texrender is there,
 * wanted, and in the source's format (render_item() asks
 * gs_texrender_get_format() only then). */
enum obs_scene_render_texrender obs_scene_render_texrender_action(bool has_texrender, bool use_texrender,
								  bool format_matches);

/* The source's width or height is 0. */
bool obs_scene_render_size_empty(uint32_t width, uint32_t height);

/* blend_type: item->blend_type, an enum obs_blending_type. */
bool obs_scene_render_clear_white(int blend_type);

/* What render_item() draws. */
enum obs_scene_render_child_id {
	/* the source */
	OBS_SCENE_RENDER_CHILD_SOURCE,
	/* item->show_transition */
	OBS_SCENE_RENDER_CHILD_SHOW,
	/* item->hide_transition */
	OBS_SCENE_RENDER_CHILD_HIDE,
};

/* user_visible: item->user_visible. show_active and hide_active:
 * scene_transition_active() of item->show_transition and
 * item->hide_transition (render_item() asks for the show transition only
 * when the item is visible, and for the hide transition only when it is
 * not, and passes false for the other). */
enum obs_scene_render_child_id obs_scene_render_child(bool user_visible, bool show_active, bool hide_active);

/* has_texrender: item->item_render != NULL after the texrender action.
 * blend_method: item->blend_method, an enum obs_blending_method. */
bool obs_scene_render_linear_srgb(bool has_texrender, int blend_method);

/* The matrix is the identity but for a whole-number x and y translation,
 * compared byte for byte (so -0.0 is not 0.0, and a NaN compares by its
 * bits). */
bool obs_scene_texcoords_centered(const struct matrix4 *m);

/* The effect render_item_texture() draws with: obs->video.* */
enum obs_scene_render_effect_id {
	/* default_effect */
	OBS_SCENE_RENDER_EFFECT_DEFAULT,
	/* bilinear_lowres_effect */
	OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES,
	/* bicubic_effect */
	OBS_SCENE_RENDER_EFFECT_BICUBIC,
	/* lanczos_effect */
	OBS_SCENE_RENDER_EFFECT_LANCZOS,
	/* area_effect */
	OBS_SCENE_RENDER_EFFECT_AREA,
};

struct obs_scene_render_effect_plan {
	/* an enum obs_scene_render_effect_id */
	int effect;
	/* set obs->video.point_sampler on the effect's "image" */
	bool point_sampler;
	/* set "base_dimension" and "base_dimension_i" from the texture's size */
	bool base_dimension;
	/* Area, scaled up on both axes: the DrawUpscale techniques */
	bool upscale;
};

/* scale_filter: item->scale_filter, an enum obs_scale_type. scale_x and
 * scale_y: item->output_scale. */
void obs_scene_render_effect(int scale_filter, float scale_x, float scale_y, struct obs_scene_render_effect_plan *plan);

/* current_space: the target's, source_space: the source's, enum
 * gs_color_space values. sdr_white_level: obs_get_video_sdr_white_level(). */
float obs_scene_render_multiplier(int current_space, int source_space, float sdr_white_level);

/* The technique render_item_texture() draws with. */
enum obs_scene_render_technique_id {
	/* "Draw" */
	OBS_SCENE_RENDER_TECH_DRAW,
	/* "DrawMultiply" */
	OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY,
	/* "DrawTonemap" */
	OBS_SCENE_RENDER_TECH_DRAW_TONEMAP,
	/* "DrawMultiplyTonemap" */
	OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP,
	/* "DrawUpscale" */
	OBS_SCENE_RENDER_TECH_DRAW_UPSCALE,
	/* "DrawUpscaleMultiply" */
	OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY,
	/* "DrawUpscaleTonemap" */
	OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP,
	/* "DrawUpscaleMultiplyTonemap" */
	OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP,
};

/* upscale: from obs_scene_render_effect(). current_space and source_space as
 * for the multiplier. */
enum obs_scene_render_technique_id obs_scene_render_technique(bool upscale, int current_space, int source_space);
