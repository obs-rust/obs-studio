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

#include "libobs/build/libobs-effect-parser-funcs.pch.h"

struct ep_struct *ep_getstruct(struct effect_parser *ep, const char *name)
{
	size_t i;
	for (i = 0; i < ep->structs.num; i++) {
		if (strcmp(name, ep->structs.array[i].name) == 0)
			return ep->structs.array + i;
	}

	return NULL;
}

struct ep_sampler *ep_getsampler(struct effect_parser *ep, const char *name)
{
	size_t i;
	for (i = 0; i < ep->samplers.num; i++) {
		if (strcmp(name, ep->samplers.array[i].name) == 0)
			return ep->samplers.array + i;
	}

	return NULL;
}

struct ep_func *ep_getfunc_strref(struct effect_parser *ep, const struct strref *ref)
{
	size_t i;
	for (i = 0; i < ep->funcs.num; i++) {
		if (strref_cmp(ref, ep->funcs.array[i].name) == 0)
			return ep->funcs.array + i;
	}

	return NULL;
}

static inline struct ep_struct *ep_getstruct_strref(struct effect_parser *ep, const struct strref *ref)
{
	size_t i;
	for (i = 0; i < ep->structs.num; i++) {
		if (strref_cmp(ref, ep->structs.array[i].name) == 0)
			return ep->structs.array + i;
	}

	return NULL;
}

static inline struct ep_sampler *ep_getsampler_strref(struct effect_parser *ep, const struct strref *ref)
{
	size_t i;
	for (i = 0; i < ep->samplers.num; i++) {
		if (strref_cmp(ref, ep->samplers.array[i].name) == 0)
			return ep->samplers.array + i;
	}

	return NULL;
}

struct ep_param *ep_getparam_strref(struct effect_parser *ep, const struct strref *ref)
{
	size_t i;
	for (i = 0; i < ep->params.num; i++) {
		if (strref_cmp(ref, ep->params.array[i].name) == 0)
			return ep->params.array + i;
	}

	return NULL;
}

int ep_check_for_keyword(struct effect_parser *ep, const char *keyword, bool *val)
{
	bool new_val = cf_token_is(&ep->cfp, keyword);
	if (new_val) {
		if (!cf_next_valid_token(&ep->cfp))
			return PARSE_EOF;

		if (new_val && *val)
			cf_adderror(&ep->cfp, "'$1' keyword already specified", LEX_WARNING, keyword, NULL, NULL);
		*val = new_val;

		return PARSE_CONTINUE;
	}

	return PARSE_SUCCESS;
}

static inline int ep_parse_func_param(struct effect_parser *ep, struct ep_func *func, struct ep_var *var)
{
	int code;
	bool var_type_keyword = false;

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	code = ep_check_for_keyword(ep, "in", &var_type_keyword);
	if (code == PARSE_EOF)
		return PARSE_EOF;
	else if (var_type_keyword)
		var->var_type = EP_VAR_IN;

	if (!var_type_keyword) {
		code = ep_check_for_keyword(ep, "inout", &var_type_keyword);
		if (code == PARSE_EOF)
			return PARSE_EOF;
		else if (var_type_keyword)
			var->var_type = EP_VAR_INOUT;
	}

	if (!var_type_keyword) {
		code = ep_check_for_keyword(ep, "out", &var_type_keyword);
		if (code == PARSE_EOF)
			return PARSE_EOF;
		else if (var_type_keyword)
			var->var_type = EP_VAR_OUT;
	}

	if (!var_type_keyword) {
		code = ep_check_for_keyword(ep, "uniform", &var_type_keyword);
		if (code == PARSE_EOF)
			return PARSE_EOF;
		else if (var_type_keyword)
			var->var_type = EP_VAR_UNIFORM;
	}

	code = cf_get_name(&ep->cfp, &var->type, "type", ")");
	if (code != PARSE_SUCCESS)
		return code;

	code = cf_next_name(&ep->cfp, &var->name, "name", ")");
	if (code != PARSE_SUCCESS)
		return code;

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, ":")) {
		code = cf_next_name(&ep->cfp, &var->mapping, "mapping specifier", ")");
		if (code != PARSE_SUCCESS)
			return code;

		if (!cf_next_valid_token(&ep->cfp))
			return PARSE_EOF;
	}

	if (ep_getstruct(ep, var->type) != NULL)
		da_push_back(func->struct_deps, &var->type);
	else if (ep_getsampler(ep, var->type) != NULL)
		da_push_back(func->sampler_deps, &var->type);

	return PARSE_SUCCESS;
}

static bool ep_parse_func_params(struct effect_parser *ep, struct ep_func *func)
{
	struct cf_token peek;
	int code;

	cf_token_clear(&peek);

	if (!cf_peek_valid_token(&ep->cfp, &peek))
		return false;

	if (*peek.str.array == ')') {
		cf_next_token(&ep->cfp);
		goto exit;
	}

	do {
		struct ep_var var;
		ep_var_init(&var);

		if (!cf_token_is(&ep->cfp, "(") && !cf_token_is(&ep->cfp, ","))
			cf_adderror_syntax_error(&ep->cfp);

		code = ep_parse_func_param(ep, func, &var);
		if (code != PARSE_SUCCESS) {
			ep_var_free(&var);

			if (code == PARSE_CONTINUE)
				goto exit;
			else if (code == PARSE_EOF)
				return false;
		}

		da_push_back(func->param_vars, &var);
	} while (!cf_token_is(&ep->cfp, ")"));

exit:
	return true;
}

static inline bool ep_process_struct_dep(struct effect_parser *ep, struct ep_func *func)
{
	struct ep_struct *val = ep_getstruct_strref(ep, &ep->cfp.cur_token->str);
	if (val)
		da_push_back(func->struct_deps, &val->name);
	return val != NULL;
}

static inline bool ep_process_func_dep(struct effect_parser *ep, struct ep_func *func)
{
	struct ep_func *val = ep_getfunc_strref(ep, &ep->cfp.cur_token->str);
	if (val)
		da_push_back(func->func_deps, &val->name);
	return val != NULL;
}

static inline bool ep_process_sampler_dep(struct effect_parser *ep, struct ep_func *func)
{
	struct ep_sampler *val = ep_getsampler_strref(ep, &ep->cfp.cur_token->str);
	if (val)
		da_push_back(func->sampler_deps, &val->name);
	return val != NULL;
}

static inline bool ep_process_param_dep(struct effect_parser *ep, struct ep_func *func)
{
	struct ep_param *val = ep_getparam_strref(ep, &ep->cfp.cur_token->str);
	if (val)
		da_push_back(func->param_deps, &val->name);
	return val != NULL;
}

static inline bool ep_parse_func_contents(struct effect_parser *ep, struct ep_func *func)
{
	int braces = 1;

	dstr_cat_strref(&func->contents, &ep->cfp.cur_token->str);

	while (braces > 0) {
		if ((ep->cfp.cur_token++)->type == CFTOKEN_NONE)
			return false;

		if (ep->cfp.cur_token->type == CFTOKEN_SPACETAB || ep->cfp.cur_token->type == CFTOKEN_NEWLINE) {
		} else if (cf_token_is(&ep->cfp, "{")) {
			braces++;
		} else if (cf_token_is(&ep->cfp, "}")) {
			braces--;
		} else if (ep_process_struct_dep(ep, func) || ep_process_func_dep(ep, func) ||
			   ep_process_sampler_dep(ep, func) || ep_process_param_dep(ep, func)) {
		}

		dstr_cat_strref(&func->contents, &ep->cfp.cur_token->str);
	}

	return true;
}

void ep_parse_function(struct effect_parser *ep, char *type, char *name)
{
	struct ep_func func;
	int code;

	ep_func_init(&func, type, name);
	if (ep_getstruct(ep, type))
		da_push_back(func.struct_deps, &func.ret_type);

	if (!ep_parse_func_params(ep, &func))
		goto error;

	if (!cf_next_valid_token(&ep->cfp))
		goto error;

	/* if function is mapped to something, for example COLOR */
	if (cf_token_is(&ep->cfp, ":")) {
		code = cf_next_name(&ep->cfp, &func.mapping, "mapping specifier", "{");
		if (code == PARSE_EOF)
			goto error;
		else if (code != PARSE_CONTINUE) {
			if (!cf_next_valid_token(&ep->cfp))
				goto error;
		}
	}

	if (!cf_token_is(&ep->cfp, "{")) {
		cf_adderror_expecting(&ep->cfp, "{");
		goto error;
	}

	if (!ep_parse_func_contents(ep, &func))
		goto error;

	/* it is established that the current token is '}' if we reach this */
	cf_next_token(&ep->cfp);

	da_push_back(ep->funcs, &func);
	return;

error:
	ep_func_free(&func);
}
