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

#include "libobs/build/libobs-ported-graphics.pch.h"

/* The parameter values of an effect: the gs_effect_set_* setters, the
 * getters, gs_effect_set_default and gs_effect_set_next_sampler, on
 * struct gs_effect_param. None of them touches the graphics subsystem.
 * Ported to rust/obs-graphics (effect_params): the hybrid libobs drops this
 * file. */

#ifndef min
#define min(a, b) (((a) < (b)) ? (a) : (b))
#define EFFECT_DEFINED_MIN
#endif
static inline void effect_setval_inline(gs_eparam_t *param, const void *data, size_t size)
{
	bool size_changed;

	if (!param) {
		blog(LOG_ERROR, "effect_setval_inline: invalid param");
		return;
	}

	if (!data) {
		blog(LOG_ERROR, "effect_setval_inline: invalid data");
		return;
	}

	size_changed = param->cur_val.num != size;

	if (size_changed)
		da_resize(param->cur_val, size);

	if (size_changed || memcmp(param->cur_val.array, data, size) != 0) {
		memcpy(param->cur_val.array, data, size);
		param->changed = true;
	}
}

static inline void effect_getval_inline(gs_eparam_t *param, void *data, size_t size)
{
	if (!param) {
		blog(LOG_ERROR, "effect_getval_inline: invalid param");
		return;
	}

	if (!data) {
		blog(LOG_ERROR, "effect_getval_inline: invalid data");
		return;
	}

	size_t bytes = min(size, param->cur_val.num);

	memcpy(data, param->cur_val.array, bytes);
}

static inline void effect_getdefaultval_inline(gs_eparam_t *param, void *data, size_t size)
{
	if (!param) {
		blog(LOG_ERROR, "effect_getdefaultval_inline: invalid param");
		return;
	}

	if (!data) {
		blog(LOG_ERROR, "effect_getdefaultval_inline: invalid data");
		return;
	}

	size_t bytes = min(size, param->default_val.num);

	memcpy(data, param->default_val.array, bytes);
}

void gs_effect_set_bool(gs_eparam_t *param, bool val)
{
	int b_val = (int)val;
	effect_setval_inline(param, &b_val, sizeof(int));
}

void gs_effect_set_float(gs_eparam_t *param, float val)
{
	effect_setval_inline(param, &val, sizeof(float));
}

void gs_effect_set_int(gs_eparam_t *param, int val)
{
	effect_setval_inline(param, &val, sizeof(int));
}

void gs_effect_set_matrix4(gs_eparam_t *param, const struct matrix4 *val)
{
	effect_setval_inline(param, val, sizeof(struct matrix4));
}

void gs_effect_set_vec2(gs_eparam_t *param, const struct vec2 *val)
{
	effect_setval_inline(param, val, sizeof(struct vec2));
}

void gs_effect_set_vec3(gs_eparam_t *param, const struct vec3 *val)
{
	effect_setval_inline(param, val, sizeof(float) * 3);
}

void gs_effect_set_vec4(gs_eparam_t *param, const struct vec4 *val)
{
	effect_setval_inline(param, val, sizeof(struct vec4));
}

void gs_effect_set_color(gs_eparam_t *param, uint32_t argb)
{
	struct vec4 v_color;
	vec4_from_bgra(&v_color, argb);
	effect_setval_inline(param, &v_color, sizeof(struct vec4));
}

void gs_effect_set_texture(gs_eparam_t *param, gs_texture_t *val)
{
	struct gs_shader_texture shader_tex;
	shader_tex.tex = val;
	shader_tex.srgb = false;
	effect_setval_inline(param, &shader_tex, sizeof(shader_tex));
}

void gs_effect_set_texture_srgb(gs_eparam_t *param, gs_texture_t *val)
{
	struct gs_shader_texture shader_tex;
	shader_tex.tex = val;
	shader_tex.srgb = true;
	effect_setval_inline(param, &shader_tex, sizeof(shader_tex));
}

void gs_effect_set_val(gs_eparam_t *param, const void *val, size_t size)
{
	effect_setval_inline(param, val, size);
}

void *gs_effect_get_val(gs_eparam_t *param)
{
	if (!param) {
		blog(LOG_ERROR, "gs_effect_get_val: invalid param");
		return NULL;
	}
	size_t size = param->cur_val.num;
	void *data;

	if (size)
		data = (void *)bzalloc(size);
	else
		return NULL;

	effect_getval_inline(param, data, size);

	return data;
}

size_t gs_effect_get_val_size(gs_eparam_t *param)
{
	return param ? param->cur_val.num : 0;
}

void *gs_effect_get_default_val(gs_eparam_t *param)
{
	if (!param) {
		blog(LOG_ERROR, "gs_effect_get_default_val: invalid param");
		return NULL;
	}
	size_t size = param->default_val.num;
	void *data;

	if (size)
		data = (void *)bzalloc(size);
	else
		return NULL;

	effect_getdefaultval_inline(param, data, size);

	return data;
}

size_t gs_effect_get_default_val_size(gs_eparam_t *param)
{
	return param ? param->default_val.num : 0;
}

void gs_effect_set_default(gs_eparam_t *param)
{
	/* the error the other setters log through effect_setval_inline() */
	if (!param) {
		blog(LOG_ERROR, "effect_setval_inline: invalid param");
		return;
	}

	effect_setval_inline(param, param->default_val.array, param->default_val.num);
}

void gs_effect_set_next_sampler(gs_eparam_t *param, gs_samplerstate_t *sampler)
{
	if (!param) {
		blog(LOG_ERROR, "gs_effect_set_next_sampler: invalid param");
		return;
	}

	if (param->type == GS_SHADER_PARAM_TEXTURE)
		param->next_sampler = sampler;
}

/* Keep this file's min() out of the sources a unity build compiles after it. */
#ifdef EFFECT_DEFINED_MIN
#undef min
#undef EFFECT_DEFINED_MIN
#endif
