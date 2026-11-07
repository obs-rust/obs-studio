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

#include "libobs/build/libobs-ported-effect-parser-writer.pch.h"

static inline struct ep_func *ep_getfunc(struct effect_parser *ep, const char *name)
{
	size_t i;
	for (i = 0; i < ep->funcs.num; i++) {
		if (strcmp(name, ep->funcs.array[i].name) == 0)
			return ep->funcs.array + i;
	}

	return NULL;
}

static inline struct ep_param *ep_getparam(struct effect_parser *ep, const char *name)
{
	size_t i;
	for (i = 0; i < ep->params.num; i++) {
		if (strcmp(name, ep->params.array[i].name) == 0)
			return ep->params.array + i;
	}

	return NULL;
}

static inline struct ep_param *ep_getannotation(struct ep_param *param, const char *name)
{
	size_t i;
	for (i = 0; i < param->annotations.num; i++) {
		if (strcmp(name, param->annotations.array[i].name) == 0)
			return param->annotations.array + i;
	}

	return NULL;
}

static inline void ep_write_param(struct dstr *shader, struct ep_param *param, dstr_array_t *used_params)
{
	if (param->written)
		return;

	if (param->is_const) {
		dstr_cat(shader, "const ");
	} else if (param->is_uniform) {
		struct dstr new;
		dstr_init_copy(&new, param->name);
		da_push_back(*used_params, &new);

		dstr_cat(shader, "uniform ");
	}

	dstr_cat(shader, param->type);
	dstr_cat(shader, " ");
	dstr_cat(shader, param->name);

	if (param->array_count)
		dstr_catf(shader, "[%u]", param->array_count);

	dstr_cat(shader, ";\n");

	param->written = true;
}

static inline void ep_write_func_param_deps(struct effect_parser *ep, struct dstr *shader, struct ep_func *func,
					    dstr_array_t *used_params)
{
	size_t i;
	for (i = 0; i < func->param_deps.num; i++) {
		const char *name = func->param_deps.array[i];
		struct ep_param *param = ep_getparam(ep, name);
		ep_write_param(shader, param, used_params);
	}

	if (func->param_deps.num)
		dstr_cat(shader, "\n\n");
}

static void ep_write_sampler(struct dstr *shader, struct ep_sampler *sampler)
{
	size_t i;

	if (sampler->written)
		return;

	dstr_cat(shader, "sampler_state ");
	dstr_cat(shader, sampler->name);
	dstr_cat(shader, " {");

	for (i = 0; i < sampler->values.num; i++) {
		dstr_cat(shader, "\n\t");
		dstr_cat(shader, sampler->states.array[i]);
		dstr_cat(shader, " = ");
		dstr_cat(shader, sampler->values.array[i]);
		dstr_cat(shader, ";\n");
	}

	dstr_cat(shader, "\n};\n");
	sampler->written = true;
}

static inline void ep_write_func_sampler_deps(struct effect_parser *ep, struct dstr *shader, struct ep_func *func)
{
	size_t i;
	for (i = 0; i < func->sampler_deps.num; i++) {
		const char *name = func->sampler_deps.array[i];

		struct ep_sampler *sampler = ep_getsampler(ep, name);
		ep_write_sampler(shader, sampler);
		dstr_cat(shader, "\n");
	}
}

static inline void ep_write_var(struct dstr *shader, struct ep_var *var)
{
	if (var->var_type == EP_VAR_INOUT)
		dstr_cat(shader, "inout ");
	else if (var->var_type == EP_VAR_OUT)
		dstr_cat(shader, "out ");
	else if (var->var_type == EP_VAR_UNIFORM)
		dstr_cat(shader, "uniform ");
	// The "in" input modifier is implied by default, so leave it blank
	// in that case.

	dstr_cat(shader, var->type);
	dstr_cat(shader, " ");
	dstr_cat(shader, var->name);

	if (var->mapping) {
		dstr_cat(shader, " : ");
		dstr_cat(shader, var->mapping);
	}
}

static void ep_write_struct(struct dstr *shader, struct ep_struct *st)
{
	size_t i;

	if (st->written)
		return;

	dstr_cat(shader, "struct ");
	dstr_cat(shader, st->name);
	dstr_cat(shader, " {");

	for (i = 0; i < st->vars.num; i++) {
		dstr_cat(shader, "\n\t");
		ep_write_var(shader, st->vars.array + i);
		dstr_cat(shader, ";");
	}

	dstr_cat(shader, "\n};\n");
	st->written = true;
}

static inline void ep_write_func_struct_deps(struct effect_parser *ep, struct dstr *shader, struct ep_func *func)
{
	size_t i;
	for (i = 0; i < func->struct_deps.num; i++) {
		const char *name = func->struct_deps.array[i];
		struct ep_struct *st = ep_getstruct(ep, name);

		if (!st->written) {
			ep_write_struct(shader, st);
			dstr_cat(shader, "\n");
			st->written = true;
		}
	}
}

static void ep_write_func(struct effect_parser *ep, struct dstr *shader, struct ep_func *func,
			  dstr_array_t *used_params);

static inline void ep_write_func_func_deps(struct effect_parser *ep, struct dstr *shader, struct ep_func *func,
					   dstr_array_t *used_params)
{
	size_t i;
	for (i = 0; i < func->func_deps.num; i++) {
		const char *name = func->func_deps.array[i];
		struct ep_func *func_dep = ep_getfunc(ep, name);

		if (!func_dep->written) {
			ep_write_func(ep, shader, func_dep, used_params);
			dstr_cat(shader, "\n\n");
		}
	}
}

static void ep_write_func(struct effect_parser *ep, struct dstr *shader, struct ep_func *func,
			  dstr_array_t *used_params)
{
	size_t i;

	func->written = true;

	ep_write_func_param_deps(ep, shader, func, used_params);
	ep_write_func_sampler_deps(ep, shader, func);
	ep_write_func_struct_deps(ep, shader, func);
	ep_write_func_func_deps(ep, shader, func, used_params);

	/* ------------------------------------ */

	dstr_cat(shader, func->ret_type);
	dstr_cat(shader, " ");
	dstr_cat(shader, func->name);
	dstr_cat(shader, "(");

	for (i = 0; i < func->param_vars.num; i++) {
		struct ep_var *var = func->param_vars.array + i;

		if (i)
			dstr_cat(shader, ", ");
		ep_write_var(shader, var);
	}

	dstr_cat(shader, ")\n");
	dstr_cat_dstr(shader, &func->contents);
	dstr_cat(shader, "\n");
}

/* writes mapped vars used by the call as parameters for main */
static void ep_write_main_params(struct effect_parser *ep, struct dstr *shader, struct dstr *param_str,
				 struct ep_func *func)
{
	size_t i;
	bool empty_params = dstr_is_empty(param_str);

	for (i = 0; i < func->param_vars.num; i++) {
		struct ep_var *var = func->param_vars.array + i;
		struct ep_struct *st = NULL;
		bool mapped = (var->mapping != NULL);

		if (!mapped) {
			st = ep_getstruct(ep, var->type);
			if (st)
				mapped = ep_struct_mapped(st);
		}

		if (mapped) {
			dstr_cat(shader, var->type);
			dstr_cat(shader, " ");
			dstr_cat(shader, var->name);

			if (!st) {
				dstr_cat(shader, " : ");
				dstr_cat(shader, var->mapping);
			}

			if (!dstr_is_empty(param_str))
				dstr_cat(param_str, ", ");
			dstr_cat(param_str, var->name);
		}
	}

	if (!empty_params)
		dstr_cat(param_str, ", ");
}

static void ep_write_main(struct effect_parser *ep, struct dstr *shader, struct ep_func *func, struct dstr *call_str)
{
	struct dstr param_str;
	struct dstr adjusted_call;

	dstr_init(&param_str);
	dstr_init_copy_dstr(&adjusted_call, call_str);

	dstr_cat(shader, "\n");
	dstr_cat(shader, func->ret_type);
	dstr_cat(shader, " main(");

	ep_write_main_params(ep, shader, &param_str, func);

	dstr_cat(shader, ")");
	if (func->mapping) {
		dstr_cat(shader, " : ");
		dstr_cat(shader, func->mapping);
	}

	dstr_cat(shader, "\n{\n\treturn ");
	dstr_cat_dstr(shader, &adjusted_call);
	dstr_cat(shader, "\n}\n");

	dstr_free(&adjusted_call);
	dstr_free(&param_str);
}

static inline void ep_reset_written(struct effect_parser *ep)
{
	size_t i;
	for (i = 0; i < ep->params.num; i++)
		ep->params.array[i].written = false;
	for (i = 0; i < ep->structs.num; i++)
		ep->structs.array[i].written = false;
	for (i = 0; i < ep->funcs.num; i++)
		ep->funcs.array[i].written = false;
	for (i = 0; i < ep->samplers.num; i++)
		ep->samplers.array[i].written = false;
}

void ep_makeshaderstring(struct effect_parser *ep, struct dstr *shader, cf_token_array_t *shader_call,
			 dstr_array_t *used_params)
{
	struct cf_token *token = shader_call->array;
	struct cf_token *func_name;
	struct ep_func *func;
	struct dstr call_str;

	dstr_init(&call_str);

	if (!token)
		return;

	while (token->type != CFTOKEN_NONE && is_whitespace(*token->str.array))
		token++;

	if (token->type == CFTOKEN_NONE || strref_cmp(&token->str, "NULL") == 0)
		return;

	func_name = token;

	while (token->type != CFTOKEN_NONE) {
		struct ep_param *param = ep_getparam_strref(ep, &token->str);
		if (param)
			ep_write_param(shader, param, used_params);

		dstr_cat_strref(&call_str, &token->str);
		token++;
	}

	func = ep_getfunc_strref(ep, &func_name->str);
	if (func) {
		ep_write_func(ep, shader, func, used_params);
		ep_write_main(ep, shader, func, &call_str);
	}

	dstr_free(&call_str);

	ep_reset_written(ep);
}
