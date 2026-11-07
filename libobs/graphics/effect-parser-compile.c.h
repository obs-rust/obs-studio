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

#include "libobs/build/libobs-effect-parser-compile.pch.h"

static void ep_compile_annotations(ep_param_array_t *ep_annotations, gs_effect_param_array_t *gsp_annotations,
				   struct effect_parser *ep)
{
	da_resize(*gsp_annotations, ep_annotations->num);

	size_t i;
	for (i = 0; i < ep_annotations->num; i++) {
		struct gs_effect_param *param = gsp_annotations->array + i;
		struct ep_param *param_in = ep_annotations->array + i;

		param->name = bstrdup(param_in->name);
		param->section = EFFECT_ANNOTATION;
		param->effect = ep->effect;
		da_move(param->default_val, param_in->default_val);

		param->type = obs_effect_param_type(param_in->type);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
		debug_param(param, param_in, i, "\t\t");
#endif
	}
}

static void ep_compile_param_annotations(struct ep_param *ep_param_input, struct gs_effect_param *gs_effect_input,
					 struct effect_parser *ep)
{
	ep_compile_annotations(&(ep_param_input->annotations), &(gs_effect_input->annotations), ep);
}

static void ep_compile_param(struct effect_parser *ep, size_t idx)
{
	struct gs_effect_param *param;
	struct ep_param *param_in;

	param = ep->effect->params.array + idx;
	param_in = ep->params.array + idx;
	param_in->param = param;

	param->name = bstrdup(param_in->name);
	param->section = EFFECT_PARAM;
	param->effect = ep->effect;
	da_move(param->default_val, param_in->default_val);

	param->type = obs_effect_param_type(param_in->type);

	if (strcmp(param_in->name, "ViewProj") == 0)
		ep->effect->view_proj = param;
	else if (strcmp(param_in->name, "World") == 0)
		ep->effect->world = param;

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
	debug_param(param, param_in, idx, "\t");
#endif

	ep_compile_param_annotations(param_in, param, ep);
}

static bool ep_compile_pass_shaderparams(struct effect_parser *ep, pass_shaderparam_array_t *pass_params,
					 dstr_array_t *used_params, gs_shader_t *shader)
{
	size_t i;
	da_resize(*pass_params, used_params->num);

	for (i = 0; i < pass_params->num; i++) {
		struct dstr *param_name = used_params->array + i;
		struct pass_shaderparam *param = pass_params->array + i;

		param->eparam = gs_effect_get_param_by_name(ep->effect, param_name->array);
		param->sparam = gs_shader_get_param_by_name(shader, param_name->array);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
		debug_param(param->eparam, 0, i, "\t\t\t\t");
#endif

		if (!param->sparam) {
			blog(LOG_ERROR, "Effect shader parameter not found");
			return false;
		}
	}

	return true;
}

static inline bool ep_compile_pass_shader(struct effect_parser *ep, struct gs_effect_technique *tech,
					  struct gs_effect_pass *pass, struct ep_pass *pass_in, size_t pass_idx,
					  enum gs_shader_type type)
{
	struct dstr shader_str;
	struct dstr location;
	dstr_array_t used_params;
	pass_shaderparam_array_t *pass_params = NULL;
	gs_shader_t *shader = NULL;
	bool success = true;
	char *errors = NULL;

	dstr_init(&shader_str);
	da_init(used_params);
	dstr_init(&location);

	dstr_copy(&location, ep->cfp.lex.file);
	if (type == GS_SHADER_VERTEX)
		dstr_cat(&location, " (Vertex ");
	else if (type == GS_SHADER_PIXEL)
		dstr_cat(&location, " (Pixel ");
	/*else if (type == SHADER_GEOMETRY)
		dstr_cat(&location, " (Geometry ");*/

	assert(pass_idx <= UINT_MAX);
	dstr_catf(&location, "shader, technique %s, pass %u)", tech->name, (unsigned)pass_idx);

	if (type == GS_SHADER_VERTEX) {
		ep_makeshaderstring(ep, &shader_str, &pass_in->vertex_program, &used_params);

		pass->vertshader = gs_vertexshader_create(shader_str.array, location.array, &errors);

		shader = pass->vertshader;
		pass_params = &pass->vertshader_params;
	} else if (type == GS_SHADER_PIXEL) {
		ep_makeshaderstring(ep, &shader_str, &pass_in->fragment_program, &used_params);

		pass->pixelshader = gs_pixelshader_create(shader_str.array, location.array, &errors);

		shader = pass->pixelshader;
		pass_params = &pass->pixelshader_params;
	}

	if (errors && strlen(errors)) {
		cf_adderror(&ep->cfp, "Error creating shader: $1", LEX_ERROR, errors, NULL, NULL);
	}
	bfree(errors);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
	blog(LOG_DEBUG, "\t\t\t%s Shader:", type == GS_SHADER_VERTEX ? "Vertex" : "Fragment");
	blog(LOG_DEBUG, "\t\t\tCode:");
	debug_print_string("\t\t\t\t\t", shader_str.array);
	blog(LOG_DEBUG, "\t\t\tParameters:");
#endif

	if (shader)
		success = ep_compile_pass_shaderparams(ep, pass_params, &used_params, shader);
	else
		success = false;

	dstr_free(&location);
	dstr_array_free(used_params.array, used_params.num);
	da_free(used_params);
	dstr_free(&shader_str);

	return success;
}

static bool ep_compile_pass(struct effect_parser *ep, struct gs_effect_technique *tech, struct ep_technique *tech_in,
			    size_t idx)
{
	struct gs_effect_pass *pass;
	struct ep_pass *pass_in;
	bool success = true;

	pass = tech->passes.array + idx;
	pass_in = tech_in->passes.array + idx;

	pass->name = bstrdup(pass_in->name);
	pass->section = EFFECT_PASS;

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
	blog(LOG_DEBUG, "\t\t[%4lld] Pass '%s':", idx, pass->name);
#endif

	if (!ep_compile_pass_shader(ep, tech, pass, pass_in, idx, GS_SHADER_VERTEX)) {
		success = false;
		blog(LOG_ERROR, "Pass (%zu) <%s> missing vertex shader!", idx, pass->name ? pass->name : "");
	}
	if (!ep_compile_pass_shader(ep, tech, pass, pass_in, idx, GS_SHADER_PIXEL)) {
		success = false;
		blog(LOG_ERROR, "Pass (%zu) <%s> missing pixel shader!", idx, pass->name ? pass->name : "");
	}
	return success;
}

static inline bool ep_compile_technique(struct effect_parser *ep, size_t idx)
{
	struct gs_effect_technique *tech;
	struct ep_technique *tech_in;
	bool success = true;
	size_t i;

	tech = ep->effect->techniques.array + idx;
	tech_in = ep->techniques.array + idx;

	tech->name = bstrdup(tech_in->name);
	tech->section = EFFECT_TECHNIQUE;
	tech->effect = ep->effect;

	da_resize(tech->passes, tech_in->passes.num);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
	blog(LOG_DEBUG, "\t[%4lld] Technique '%s' has %lld passes:", idx, tech->name, tech->passes.num);
#endif

	for (i = 0; i < tech->passes.num; i++) {
		if (!ep_compile_pass(ep, tech, tech_in, i))
			success = false;
	}

	return success;
}

bool ep_compile(struct effect_parser *ep)
{
	bool success = true;
	size_t i;

	assert(ep->effect);

	da_resize(ep->effect->params, ep->params.num);
	da_resize(ep->effect->techniques, ep->techniques.num);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
	blog(LOG_DEBUG, "Shader has %lld parameters:", ep->params.num);
#endif

	for (i = 0; i < ep->params.num; i++)
		ep_compile_param(ep, i);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
	blog(LOG_DEBUG, "Shader has %lld techniques:", ep->techniques.num);
#endif

	for (i = 0; i < ep->techniques.num; i++) {
		if (!ep_compile_technique(ep, i))
			success = false;
	}

	return success;
}
