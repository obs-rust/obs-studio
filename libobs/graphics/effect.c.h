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

#include "build/libobs.pch.h"

void gs_effect_actually_destroy(gs_effect_t *effect)
{
	effect_free(effect);
	bfree(effect);
}

void gs_effect_destroy(gs_effect_t *effect)
{
	if (effect) {
		if (!effect->cached)
			gs_effect_actually_destroy(effect);
	}
}

bool gs_effect_loop(gs_effect_t *effect, const char *name)
{
	if (!effect) {
		return false;
	}

	if (!effect->looping) {
		gs_technique_t *tech;

		if (!!gs_get_effect()) {
			blog(LOG_WARNING, "gs_effect_loop: An effect is "
					  "already active");
			return false;
		}

		tech = gs_effect_get_technique(effect, name);
		if (!tech) {
			blog(LOG_WARNING,
			     "gs_effect_loop: Technique '%s' "
			     "not found.",
			     name);
			return false;
		}

		gs_technique_begin(tech);

		effect->looping = true;
	} else {
		gs_technique_end_pass(effect->cur_technique);
	}

	if (!gs_technique_begin_pass(effect->cur_technique, effect->loop_pass++)) {
		gs_technique_end(effect->cur_technique);
		effect->looping = false;
		effect->loop_pass = 0;
		return false;
	}

	return true;
}

size_t gs_technique_begin(gs_technique_t *tech)
{
	if (!tech)
		return 0;

	tech->effect->cur_technique = tech;
	tech->effect->graphics->cur_effect = tech->effect;

	return tech->passes.num;
}

void gs_technique_end(gs_technique_t *tech)
{
	if (!tech)
		return;

	struct gs_effect *effect = tech->effect;
	struct gs_effect_param *params = effect->params.array;
	size_t i;

	gs_load_vertexshader(NULL);
	gs_load_pixelshader(NULL);

	tech->effect->cur_technique = NULL;
	tech->effect->graphics->cur_effect = NULL;

	for (i = 0; i < effect->params.num; i++) {
		struct gs_effect_param *param = params + i;

		da_resize(param->cur_val, 0);
		param->changed = false;
		if (param->next_sampler)
			param->next_sampler = NULL;
	}
}

static inline void reset_params(pass_shaderparam_array_t *shaderparams)
{
	struct pass_shaderparam *params = shaderparams->array;
	size_t i;

	for (i = 0; i < shaderparams->num; i++)
		params[i].eparam->changed = false;
}

static void upload_shader_params(pass_shaderparam_array_t *pass_params, bool changed_only)
{
	struct pass_shaderparam *params = pass_params->array;
	size_t i;

	for (i = 0; i < pass_params->num; i++) {
		struct pass_shaderparam *param = params + i;
		struct gs_effect_param *eparam = param->eparam;
		gs_sparam_t *sparam = param->sparam;

		if (eparam->next_sampler)
			gs_shader_set_next_sampler(sparam, eparam->next_sampler);

		if (changed_only && !eparam->changed)
			continue;

		if (!eparam->cur_val.num) {
			if (eparam->default_val.num)
				da_copy(eparam->cur_val, eparam->default_val);
			else
				continue;
		}

		gs_shader_set_val(sparam, eparam->cur_val.array, eparam->cur_val.num);
	}
}

static inline void upload_parameters(struct gs_effect *effect, bool changed_only)
{
	pass_shaderparam_array_t *vshader_params, *pshader_params;

	if (!effect->cur_pass)
		return;

	vshader_params = &effect->cur_pass->vertshader_params;
	pshader_params = &effect->cur_pass->pixelshader_params;

	upload_shader_params(vshader_params, changed_only);
	upload_shader_params(pshader_params, changed_only);
	reset_params(vshader_params);
	reset_params(pshader_params);
}

void gs_effect_update_params(gs_effect_t *effect)
{
	if (effect)
		upload_parameters(effect, true);
}

bool gs_technique_begin_pass(gs_technique_t *tech, size_t idx)
{
	struct gs_effect_pass *passes;
	struct gs_effect_pass *cur_pass;

	if (!tech || idx >= tech->passes.num)
		return false;

	passes = tech->passes.array;
	cur_pass = passes + idx;

	tech->effect->cur_pass = cur_pass;
	gs_load_vertexshader(cur_pass->vertshader);
	gs_load_pixelshader(cur_pass->pixelshader);
	upload_parameters(tech->effect, false);

	return true;
}

bool gs_technique_begin_pass_by_name(gs_technique_t *tech, const char *name)
{
	if (!tech)
		return false;

	for (size_t i = 0; i < tech->passes.num; i++) {
		struct gs_effect_pass *pass = tech->passes.array + i;
		if (strcmp(pass->name, name) == 0) {
			gs_technique_begin_pass(tech, i);
			return true;
		}
	}

	return false;
}

static inline void clear_tex_params(pass_shaderparam_array_t *in_params)
{
	struct pass_shaderparam *params = in_params->array;

	for (size_t i = 0; i < in_params->num; i++) {
		struct pass_shaderparam *param = params + i;
		struct gs_shader_param_info info;

		gs_shader_get_param_info(param->sparam, &info);
		if (info.type == GS_SHADER_PARAM_TEXTURE)
			gs_shader_set_texture(param->sparam, NULL);
	}
}

void gs_technique_end_pass(gs_technique_t *tech)
{
	if (!tech)
		return;

	struct gs_effect_pass *pass = tech->effect->cur_pass;
	if (!pass)
		return;

	clear_tex_params(&pass->vertshader_params);
	clear_tex_params(&pass->pixelshader_params);
	tech->effect->cur_pass = NULL;
}

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

#ifndef min
#define min(a, b) (((a) < (b)) ? (a) : (b))
#define EFFECT_DEFINED_MIN
#endif
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
