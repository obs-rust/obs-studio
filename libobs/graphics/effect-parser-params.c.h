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

#include "libobs/build/libobs-effect-parser-params.pch.h"

static inline int ep_parse_param_annotation_var(struct effect_parser *ep, struct ep_param *var)
{
	int code;

	/* -------------------------------------- */
	/* variable type */

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, ";"))
		return PARSE_CONTINUE;
	if (cf_token_is(&ep->cfp, ">"))
		return PARSE_BREAK;

	code = cf_token_is_type(&ep->cfp, CFTOKEN_NAME, "type name", ";");
	if (code != PARSE_SUCCESS)
		return code;

	bfree(var->type);
	cf_copy_token(&ep->cfp, &var->type);

	/* -------------------------------------- */
	/* variable name */

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, ";")) {
		cf_adderror_expecting(&ep->cfp, "variable name");
		return PARSE_UNEXPECTED_CONTINUE;
	}
	if (cf_token_is(&ep->cfp, ">")) {
		cf_adderror_expecting(&ep->cfp, "variable name");
		return PARSE_UNEXPECTED_BREAK;
	}

	code = cf_token_is_type(&ep->cfp, CFTOKEN_NAME, "variable name", ";");
	if (code != PARSE_SUCCESS)
		return code;

	bfree(var->name);
	cf_copy_token(&ep->cfp, &var->name);

	/* -------------------------------------- */
	/* variable mapping if any (POSITION, TEXCOORD, etc) */

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, ":")) {
		cf_adderror_expecting(&ep->cfp, "= or ;");
		return PARSE_UNEXPECTED_BREAK;
	} else if (cf_token_is(&ep->cfp, ">")) {
		cf_adderror_expecting(&ep->cfp, "= or ;");
		return PARSE_UNEXPECTED_BREAK;
	} else if (cf_token_is(&ep->cfp, "=")) {
		if (!ep_parse_param_assign(ep, var)) {
			cf_adderror_expecting(&ep->cfp, "assignment value");
			return PARSE_UNEXPECTED_BREAK;
		}
	}

	/* -------------------------------------- */

	if (!cf_token_is(&ep->cfp, ";")) {
		if (!cf_go_to_valid_token(&ep->cfp, ";", ">")) {
			cf_adderror_expecting(&ep->cfp, "; or >");
			return PARSE_EOF;
		}
		return PARSE_CONTINUE;
	}

	return PARSE_SUCCESS;
}

static int ep_parse_annotations(struct effect_parser *ep, ep_param_array_t *annotations)
{
	if (!cf_token_is(&ep->cfp, "<")) {
		cf_adderror_expecting(&ep->cfp, "<");
		goto error;
	}

	/* get annotation variables */
	while (true) {
		bool do_break = false;
		struct ep_param var;

		ep_param_init(&var, bstrdup(""), bstrdup(""), false, false, false);

		switch (ep_parse_param_annotation_var(ep, &var)) {
		case PARSE_UNEXPECTED_CONTINUE:
			cf_adderror_syntax_error(&ep->cfp);
			/* Falls through. */
		case PARSE_CONTINUE:
			ep_param_free(&var);
			continue;

		case PARSE_UNEXPECTED_BREAK:
			cf_adderror_syntax_error(&ep->cfp);
			/* Falls through. */
		case PARSE_BREAK:
			ep_param_free(&var);
			do_break = true;
			break;

		case PARSE_EOF:
			ep_param_free(&var);
			goto error;
		}

		if (do_break)
			break;

		da_push_back(*annotations, &var);
	}

	if (!cf_token_is(&ep->cfp, ">")) {
		cf_adderror_expecting(&ep->cfp, ">");
		goto error;
	}
	if (!cf_next_valid_token(&ep->cfp))
		goto error;

	return true;

error:
	return false;
}

static int ep_parse_param_annotations(struct effect_parser *ep, struct ep_param *param)
{
	return ep_parse_annotations(ep, &param->annotations);
}

/* parses "array[count]" */
bool ep_parse_param_array(struct effect_parser *ep, struct ep_param *param)
{
	if (!cf_next_valid_token(&ep->cfp))
		return false;

	if (ep->cfp.cur_token->type != CFTOKEN_NUM ||
	    !obs_effect_param_array_count(ep->cfp.cur_token->str.array, ep->cfp.cur_token->str.len,
					  &param->array_count))
		return false;

	if (cf_next_token_should_be(&ep->cfp, "]", ";", NULL) == PARSE_EOF)
		return false;

	if (!cf_next_valid_token(&ep->cfp))
		return false;

	return true;
}

static inline int ep_parse_param_assign_texture(struct effect_parser *ep, struct ep_param *param)
{
	int code;
	char *str;

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	code = cf_token_is_type(&ep->cfp, CFTOKEN_STRING, "texture path string", ";");
	if (code != PARSE_SUCCESS)
		return code;

	str = cf_literal_to_str(ep->cfp.cur_token->str.array, ep->cfp.cur_token->str.len);

	if (str) {
		da_copy_array(param->default_val, str, strlen(str) + 1);
		bfree(str);
	}

	return PARSE_SUCCESS;
}

static inline int ep_parse_param_assign_string(struct effect_parser *ep, struct ep_param *param)
{
	int code;
	char *str = NULL;

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	code = cf_token_is_type(&ep->cfp, CFTOKEN_STRING, "string", ";");
	if (code != PARSE_SUCCESS)
		return code;

	str = cf_literal_to_str(ep->cfp.cur_token->str.array, ep->cfp.cur_token->str.len);

	if (str) {
		da_copy_array(param->default_val, str, strlen(str) + 1);
		bfree(str);
	}

	return PARSE_SUCCESS;
}

static inline int ep_parse_param_assign_intfloat(struct effect_parser *ep, struct ep_param *param, bool is_float)
{
	int code;
	bool is_negative = false;

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, "-")) {
		is_negative = true;

		if (!cf_next_token(&ep->cfp))
			return PARSE_EOF;
	}

	code = cf_token_is_type(&ep->cfp, CFTOKEN_NUM, "numeric value", ";");
	if (code != PARSE_SUCCESS)
		return code;

	if (is_float) {
		float f = (float)os_strtod(ep->cfp.cur_token->str.array);
		if (is_negative)
			f = -f;
		da_push_back_array(param->default_val, (uint8_t *)&f, sizeof(float));
	} else {
		int i = obs_effect_param_int_value(ep->cfp.cur_token->str.array, is_negative);
		da_push_back_array(param->default_val, (uint8_t *)&i, sizeof(int));
	}

	return PARSE_SUCCESS;
}

static inline int ep_parse_param_assign_bool(struct effect_parser *ep, struct ep_param *param)
{
	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, "true")) {
		int i = 1;
		da_push_back_array(param->default_val, (uint8_t *)&i, sizeof(int));
		return PARSE_SUCCESS;
	} else if (cf_token_is(&ep->cfp, "false")) {
		int i = 0;
		da_push_back_array(param->default_val, (uint8_t *)&i, sizeof(int));
		return PARSE_SUCCESS;
	}

	cf_adderror_expecting(&ep->cfp, "true or false");

	return PARSE_EOF;
}

/*
 * parses assignment for float1, float2, float3, float4, int1, int2, int3, int4,
 * and any combination for float3x3, float4x4, int3x3, int4x4, etc
*/
static inline int ep_parse_param_assign_intfloat_array(struct effect_parser *ep, struct ep_param *param, bool is_float)
{
	int intfloat_count = 0, code, i;

	/* -------------------------------------------- */

	if (obs_effect_param_rows_bad(param->type, is_float))
		cf_adderror(&ep->cfp, "Invalid row count", LEX_ERROR, NULL, NULL, NULL);

	intfloat_count = obs_effect_param_rows(param->type, is_float);

	if (obs_effect_param_has_cols(param->type, is_float)) {
		if (obs_effect_param_cols_bad(param->type, is_float))
			cf_adderror(&ep->cfp, "Invalid column count", LEX_ERROR, NULL, NULL, NULL);

		intfloat_count = obs_effect_param_cols(param->type, is_float, intfloat_count);
	}

	/* -------------------------------------------- */
	if (!cf_next_token(&ep->cfp)) {
		cf_adderror_unexpected_eof(&ep->cfp);
		return PARSE_EOF;
	}
	char *end_char = NULL;
	if (strref_cmp(&ep->cfp.cur_token->str, "{") == 0) {
		end_char = "}";
	} else if (strref_cmp(&ep->cfp.cur_token->str, param->type) == 0) {
		end_char = ")";
		code = cf_next_token_should_be(&ep->cfp, "(", ";", NULL);
		if (code != PARSE_SUCCESS)
			return code;
	} else if (!cf_go_to_token(&ep->cfp, ";", NULL)) {
		return PARSE_EOF;
	} else {
		cf_adderror_expecting(&ep->cfp, "{");
		return PARSE_CONTINUE;
	}

	for (i = 0; i < intfloat_count; i++) {
		char *next = ((i + 1) < intfloat_count) ? "," : end_char;

		code = ep_parse_param_assign_intfloat(ep, param, is_float);
		if (code != PARSE_SUCCESS)
			return code;

		code = cf_next_token_should_be(&ep->cfp, next, ";", NULL);
		if (code != PARSE_SUCCESS)
			return code;
	}

	return PARSE_SUCCESS;
}

static int ep_parse_param_assignment_val(struct effect_parser *ep, struct ep_param *param)
{
	switch (obs_effect_param_route(param->type, param->is_texture)) {
	case OBS_EFFECT_PARAM_ROUTE_TEXTURE:
		return ep_parse_param_assign_texture(ep, param);
	case OBS_EFFECT_PARAM_ROUTE_INT:
		return ep_parse_param_assign_intfloat(ep, param, false);
	case OBS_EFFECT_PARAM_ROUTE_FLOAT:
		return ep_parse_param_assign_intfloat(ep, param, true);
	case OBS_EFFECT_PARAM_ROUTE_INT_ARRAY:
		return ep_parse_param_assign_intfloat_array(ep, param, false);
	case OBS_EFFECT_PARAM_ROUTE_FLOAT_ARRAY:
		return ep_parse_param_assign_intfloat_array(ep, param, true);
	case OBS_EFFECT_PARAM_ROUTE_STRING:
		return ep_parse_param_assign_string(ep, param);
	case OBS_EFFECT_PARAM_ROUTE_BOOL:
		return ep_parse_param_assign_bool(ep, param);
	case OBS_EFFECT_PARAM_ROUTE_INVALID:
		break;
	}

	cf_adderror(&ep->cfp, "Invalid type '$1' used for assignment", LEX_ERROR, param->type, NULL, NULL);

	return PARSE_CONTINUE;
}

bool ep_parse_param_assign(struct effect_parser *ep, struct ep_param *param)
{
	if (ep_parse_param_assignment_val(ep, param) != PARSE_SUCCESS)
		return false;

	if (!cf_next_valid_token(&ep->cfp))
		return false;

	return true;
}

/* static bool ep_parse_param_property(struct effect_parser *ep,
		struct ep_param *param)
{
} */

void ep_parse_param(struct effect_parser *ep, char *type, char *name, bool is_property, bool is_const, bool is_uniform)
{
	struct ep_param param;
	ep_param_init(&param, type, name, is_property, is_const, is_uniform);

	if (cf_token_is(&ep->cfp, ";"))
		goto complete;
	if (cf_token_is(&ep->cfp, "[") && !ep_parse_param_array(ep, &param))
		goto error;
	if (cf_token_is(&ep->cfp, "<") && !ep_parse_param_annotations(ep, &param))
		goto error;
	if (cf_token_is(&ep->cfp, "=") && !ep_parse_param_assign(ep, &param))
		goto error;
	/*
	if (cf_token_is(&ep->cfp, "<") && !ep_parse_param_property(ep, &param))
		goto error; */
	if (!cf_token_is(&ep->cfp, ";"))
		goto error;

complete:
	da_push_back(ep->params, &param);
	return;

error:
	ep_param_free(&param);
}
