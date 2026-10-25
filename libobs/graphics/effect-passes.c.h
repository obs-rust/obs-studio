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

/* The technique and pass loop of an effect (gs_effect_loop, the technique
 * and pass begin/end functions, gs_effect_update_params) and the upload of
 * the parameters to the shaders. Ported to rust/obs-graphics
 * (effect_passes): the hybrid libobs drops this file. */

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
