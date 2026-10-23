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

#include "libobs/build/libobs-ported-scene-render-plan.pch.h"

/* Ported to rust/obs-scene (render_plan): the hybrid libobs drops this file. */

/* The per-item render decisions of render_item() and render_item_texture() in
 * obs-scene.c, moved: the decisions, on plain values. obs-scene.c keeps every
 * graphics call, the source and transition renders, the texrender objects and
 * the SDR white level lookup, and makes each call where it made the decision.
 * Declared in obs-scene-render-plan.h without EXPORT. */

static inline bool render_plan_crop_enabled(int left, int top, int right, int bottom)
{
	return left || right || top || bottom;
}

bool obs_scene_render_texture_enabled(const struct obs_scene_render_triggers *t)
{
	return render_plan_crop_enabled(t->crop_left, t->crop_top, t->crop_right, t->crop_bottom) ||
	       render_plan_crop_enabled(t->bounds_crop_left, t->bounds_crop_top, t->bounds_crop_right,
					t->bounds_crop_bottom) ||
	       t->scale_filter != OBS_SCENE_RENDER_SCALE_DISABLE ||
	       (t->blend_method == OBS_SCENE_RENDER_BLEND_METHOD_SRGB_OFF) ||
	       !(t->blend_type == OBS_SCENE_RENDER_BLEND_NORMAL) || (t->is_scene && !t->is_group);
}

enum obs_scene_render_texrender obs_scene_render_texrender_action(bool has_texrender, bool use_texrender,
								  bool format_matches)
{
	/* render_item() destroyed the texrender when it was there and not
	 * wanted, or in another format, then created one when it was wanted and
	 * none was left. */
	const bool destroy = has_texrender && (!use_texrender || !format_matches);
	const bool left = has_texrender && !destroy;
	const bool create = !left && use_texrender;

	if (destroy)
		return create ? OBS_SCENE_RENDER_TEXRENDER_RECREATE : OBS_SCENE_RENDER_TEXRENDER_DESTROY;
	if (create)
		return OBS_SCENE_RENDER_TEXRENDER_CREATE;
	return left ? OBS_SCENE_RENDER_TEXRENDER_KEEP : OBS_SCENE_RENDER_TEXRENDER_NONE;
}

bool obs_scene_render_size_empty(uint32_t width, uint32_t height)
{
	return !width || !height;
}

bool obs_scene_render_clear_white(int blend_type)
{
	/* Darken draws with GS_BLEND_OP_MIN, for which the graphics APIs ignore
	 * the blend factors: a transparent pixel (0, 0, 0, 0) would always win
	 * and draw black, with alpha 0. Drawn over white instead, a transparent
	 * pixel is white with alpha 1 and leaves what is under it, and an opaque
	 * one is unchanged. */
	return blend_type == OBS_SCENE_RENDER_BLEND_DARKEN;
}

enum obs_scene_render_child_id obs_scene_render_child(bool user_visible, bool show_active, bool hide_active)
{
	if (user_visible && show_active)
		return OBS_SCENE_RENDER_CHILD_SHOW;
	else if (!user_visible && hide_active)
		return OBS_SCENE_RENDER_CHILD_HIDE;
	return OBS_SCENE_RENDER_CHILD_SOURCE;
}

bool obs_scene_render_linear_srgb(bool has_texrender, int blend_method)
{
	return !has_texrender || (blend_method != OBS_SCENE_RENDER_BLEND_METHOD_SRGB_OFF);
}

bool obs_scene_texcoords_centered(const struct matrix4 *m)
{
	static const struct matrix4 identity = {
		{1.0f, 0.0f, 0.0f, 0.0f},
		{0.0f, 1.0f, 0.0f, 0.0f},
		{0.0f, 0.0f, 1.0f, 0.0f},
		{0.0f, 0.0f, 0.0f, 1.0f},
	};
	struct matrix4 copy = identity;
	copy.t.x = floorf(m->t.x);
	copy.t.y = floorf(m->t.y);
	return memcmp(m, &copy, sizeof(*m)) == 0;
}

void obs_scene_render_effect(int scale_filter, float scale_x, float scale_y, struct obs_scene_render_effect_plan *plan)
{
	const int type = scale_filter;
	int effect = OBS_SCENE_RENDER_EFFECT_DEFAULT;
	bool point_sampler = false;
	bool base_dimension = false;
	bool upscale = false;

	if (type != OBS_SCENE_RENDER_SCALE_DISABLE) {
		if (type == OBS_SCENE_RENDER_SCALE_POINT) {
			point_sampler = true;

		} else if (!close_float(scale_x, 1.0f, EPSILON) || !close_float(scale_y, 1.0f, EPSILON)) {
			if (scale_x < 0.5f || scale_y < 0.5f) {
				effect = OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES;
			} else if (type == OBS_SCENE_RENDER_SCALE_BICUBIC) {
				effect = OBS_SCENE_RENDER_EFFECT_BICUBIC;
			} else if (type == OBS_SCENE_RENDER_SCALE_LANCZOS) {
				effect = OBS_SCENE_RENDER_EFFECT_LANCZOS;
			} else if (type == OBS_SCENE_RENDER_SCALE_AREA) {
				effect = OBS_SCENE_RENDER_EFFECT_AREA;
				upscale = (scale_x >= 1.0f) && (scale_y >= 1.0f);
			}

			base_dimension = true;
		}
	}

	plan->effect = effect;
	plan->point_sampler = point_sampler;
	plan->base_dimension = base_dimension;
	plan->upscale = upscale;
}

float obs_scene_render_multiplier(int current_space, int source_space, float sdr_white_level)
{
	float multiplier = 1.f;

	if (current_space == OBS_SCENE_RENDER_CS_709_SCRGB) {
		switch (source_space) {
		case OBS_SCENE_RENDER_CS_SRGB:
		case OBS_SCENE_RENDER_CS_SRGB_16F:
		case OBS_SCENE_RENDER_CS_709_EXTENDED:
			multiplier = sdr_white_level / 80.f;
			break;
		case OBS_SCENE_RENDER_CS_709_SCRGB:
			break;
		}
	}

	if (source_space == OBS_SCENE_RENDER_CS_709_SCRGB) {
		switch (current_space) {
		case OBS_SCENE_RENDER_CS_SRGB:
		case OBS_SCENE_RENDER_CS_SRGB_16F:
		case OBS_SCENE_RENDER_CS_709_EXTENDED:
			multiplier = 80.f / sdr_white_level;
			break;
		case OBS_SCENE_RENDER_CS_709_SCRGB:
			break;
		}
	}

	return multiplier;
}

enum obs_scene_render_technique_id obs_scene_render_technique(bool upscale, int current_space, int source_space)
{
	enum obs_scene_render_technique_id tech = OBS_SCENE_RENDER_TECH_DRAW;
	if (upscale) {
		tech = OBS_SCENE_RENDER_TECH_DRAW_UPSCALE;
		switch (source_space) {
		case OBS_SCENE_RENDER_CS_SRGB:
		case OBS_SCENE_RENDER_CS_SRGB_16F:
			if (current_space == OBS_SCENE_RENDER_CS_709_SCRGB)
				tech = OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY;
			break;
		case OBS_SCENE_RENDER_CS_709_EXTENDED:
			if (current_space == OBS_SCENE_RENDER_CS_SRGB || current_space == OBS_SCENE_RENDER_CS_SRGB_16F)
				tech = OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP;
			else if (current_space == OBS_SCENE_RENDER_CS_709_SCRGB)
				tech = OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY;
			break;
		case OBS_SCENE_RENDER_CS_709_SCRGB:
			if (current_space == OBS_SCENE_RENDER_CS_SRGB || current_space == OBS_SCENE_RENDER_CS_SRGB_16F)
				tech = OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP;
			else if (current_space == OBS_SCENE_RENDER_CS_709_EXTENDED)
				tech = OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY;
			break;
		}
	} else {
		switch (source_space) {
		case OBS_SCENE_RENDER_CS_SRGB:
		case OBS_SCENE_RENDER_CS_SRGB_16F:
			if (current_space == OBS_SCENE_RENDER_CS_709_SCRGB)
				tech = OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY;
			break;
		case OBS_SCENE_RENDER_CS_709_EXTENDED:
			if (current_space == OBS_SCENE_RENDER_CS_SRGB || current_space == OBS_SCENE_RENDER_CS_SRGB_16F)
				tech = OBS_SCENE_RENDER_TECH_DRAW_TONEMAP;
			else if (current_space == OBS_SCENE_RENDER_CS_709_SCRGB)
				tech = OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY;
			break;
		case OBS_SCENE_RENDER_CS_709_SCRGB:
			if (current_space == OBS_SCENE_RENDER_CS_SRGB || current_space == OBS_SCENE_RENDER_CS_SRGB_16F)
				tech = OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP;
			else if (current_space == OBS_SCENE_RENDER_CS_709_EXTENDED)
				tech = OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY;
			break;
		}
	}
	return tech;
}
