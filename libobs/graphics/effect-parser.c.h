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

#include "libobs/build/libobs-graphics.pch.h"

static bool ep_get_var_specifiers(struct effect_parser *ep, bool *is_property, bool *is_const, bool *is_uniform)
{
	while (true) {
		int code;
		code = ep_check_for_keyword(ep, "property", is_property);
		if (code == PARSE_EOF)
			return false;
		else if (code == PARSE_CONTINUE)
			continue;

		code = ep_check_for_keyword(ep, "const", is_const);
		if (code == PARSE_EOF)
			return false;
		else if (code == PARSE_CONTINUE)
			continue;

		code = ep_check_for_keyword(ep, "uniform", is_uniform);
		if (code == PARSE_EOF)
			return false;
		else if (code == PARSE_CONTINUE)
			continue;

		break;
	}

	return true;
}

static inline void report_invalid_func_keyword(struct effect_parser *ep, const char *name, bool val)
{
	if (val)
		cf_adderror(&ep->cfp,
			    "'$1' keyword cannot be used with a "
			    "function",
			    LEX_ERROR, name, NULL, NULL);
}

static void ep_parse_other(struct effect_parser *ep)
{
	bool is_property = false, is_const = false, is_uniform = false;
	char *type = NULL, *name = NULL;

	if (!ep_get_var_specifiers(ep, &is_property, &is_const, &is_uniform))
		goto error;

	if (cf_get_name(&ep->cfp, &type, "type", ";") != PARSE_SUCCESS)
		goto error;
	if (cf_next_name(&ep->cfp, &name, "name", ";") != PARSE_SUCCESS)
		goto error;
	if (!cf_next_valid_token(&ep->cfp))
		goto error;

	if (cf_token_is(&ep->cfp, "(")) {
		report_invalid_func_keyword(ep, "property", is_property);
		report_invalid_func_keyword(ep, "const", is_const);
		report_invalid_func_keyword(ep, "uniform", is_uniform);

		ep_parse_function(ep, type, name);
		return;
	} else {
		ep_parse_param(ep, type, name, is_property, is_const, is_uniform);
		return;
	}

error:
	bfree(type);
	bfree(name);
}

extern const char *gs_preprocessor_name(void);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
static void debug_get_default_value(struct gs_effect_param *param, char *buffer, unsigned long long buf_size)
{
	if (param->default_val.num == 0) {
		snprintf(buffer, buf_size, "(null)");
		return;
	}

	switch (param->type) {
	case GS_SHADER_PARAM_STRING:
		snprintf(buffer, buf_size, "'%.*s'", param->default_val.num, param->default_val.array);
		break;
	case GS_SHADER_PARAM_INT:
		snprintf(buffer, buf_size, "%ld", *(int *)(param->default_val.array + 0));
		break;
	case GS_SHADER_PARAM_INT2:
		snprintf(buffer, buf_size, "%ld,%ld", *(int *)(param->default_val.array + 0),
			 *(int *)(param->default_val.array + 4));
		break;
	case GS_SHADER_PARAM_INT3:
		snprintf(buffer, buf_size, "%ld,%ld,%ld", *(int *)(param->default_val.array + 0),
			 *(int *)(param->default_val.array + 4), *(int *)(param->default_val.array + 8));
		break;
	case GS_SHADER_PARAM_INT4:
		snprintf(buffer, buf_size, "%ld,%ld,%ld,%ld", *(int *)(param->default_val.array + 0),
			 *(int *)(param->default_val.array + 4), *(int *)(param->default_val.array + 8),
			 *(int *)(param->default_val.array + 12));
		break;
	case GS_SHADER_PARAM_FLOAT:
		snprintf(buffer, buf_size, "%e", *(float *)(param->default_val.array + 0));
		break;
	case GS_SHADER_PARAM_VEC2:
		snprintf(buffer, buf_size, "%e,%e", *(float *)(param->default_val.array + 0),
			 *(float *)(param->default_val.array + 4));
		break;
	case GS_SHADER_PARAM_VEC3:
		snprintf(buffer, buf_size, "%e,%e,%e", *(float *)(param->default_val.array + 0),
			 *(float *)(param->default_val.array + 4), *(float *)(param->default_val.array + 8));
		break;
	case GS_SHADER_PARAM_VEC4:
		snprintf(buffer, buf_size, "%e,%e,%e,%e", *(float *)(param->default_val.array + 0),
			 *(float *)(param->default_val.array + 4), *(float *)(param->default_val.array + 8),
			 *(float *)(param->default_val.array + 12));
		break;
	case GS_SHADER_PARAM_MATRIX4X4:
		snprintf(buffer, buf_size,
			 "[[%e,%e,%e,%e],[%e,%e,%e,%e],"
			 "[%e,%e,%e,%e],[%e,%e,%e,%e]]",
			 *(float *)(param->default_val.array + 0), *(float *)(param->default_val.array + 4),
			 *(float *)(param->default_val.array + 8), *(float *)(param->default_val.array + 12),
			 *(float *)(param->default_val.array + 16), *(float *)(param->default_val.array + 20),
			 *(float *)(param->default_val.array + 24), *(float *)(param->default_val.array + 28),
			 *(float *)(param->default_val.array + 32), *(float *)(param->default_val.array + 36),
			 *(float *)(param->default_val.array + 40), *(float *)(param->default_val.array + 44),
			 *(float *)(param->default_val.array + 48), *(float *)(param->default_val.array + 52),
			 *(float *)(param->default_val.array + 56), *(float *)(param->default_val.array + 60));
		break;
	case GS_SHADER_PARAM_BOOL:
		snprintf(buffer, buf_size, "%s", (*param->default_val.array) != 0 ? "true\0" : "false\0");
		break;
	case GS_SHADER_PARAM_UNKNOWN:
	case GS_SHADER_PARAM_TEXTURE:
		snprintf(buffer, buf_size, "<unknown>");
		break;
	}
}

void debug_param(struct gs_effect_param *param, struct ep_param *param_in, unsigned long long idx, const char *offset)
{
	char _debug_type[4096];
	switch (param->type) {
	case GS_SHADER_PARAM_STRING:
		snprintf(_debug_type, sizeof(_debug_type), "string");
		break;
	case GS_SHADER_PARAM_INT:
		snprintf(_debug_type, sizeof(_debug_type), "int");
		break;
	case GS_SHADER_PARAM_INT2:
		snprintf(_debug_type, sizeof(_debug_type), "int2");
		break;
	case GS_SHADER_PARAM_INT3:
		snprintf(_debug_type, sizeof(_debug_type), "int3");
		break;
	case GS_SHADER_PARAM_INT4:
		snprintf(_debug_type, sizeof(_debug_type), "int4");
		break;
	case GS_SHADER_PARAM_FLOAT:
		snprintf(_debug_type, sizeof(_debug_type), "float");
		break;
	case GS_SHADER_PARAM_VEC2:
		snprintf(_debug_type, sizeof(_debug_type), "float2");
		break;
	case GS_SHADER_PARAM_VEC3:
		snprintf(_debug_type, sizeof(_debug_type), "float3");
		break;
	case GS_SHADER_PARAM_VEC4:
		snprintf(_debug_type, sizeof(_debug_type), "float4");
		break;
	case GS_SHADER_PARAM_MATRIX4X4:
		snprintf(_debug_type, sizeof(_debug_type), "float4x4");
		break;
	case GS_SHADER_PARAM_BOOL:
		snprintf(_debug_type, sizeof(_debug_type), "bool");
		break;
	case GS_SHADER_PARAM_UNKNOWN:
		snprintf(_debug_type, sizeof(_debug_type), "unknown");
		break;
	case GS_SHADER_PARAM_TEXTURE:
		snprintf(_debug_type, sizeof(_debug_type), "texture");
		break;
	}

	char _debug_buf[4096];
	debug_get_default_value(param, _debug_buf, sizeof(_debug_buf));
	if (param->annotations.num > 0) {
		blog(LOG_DEBUG, "%s[%4lld] %.*s '%s' with value %.*s and %lld annotations:", offset, idx,
		     sizeof(_debug_type), _debug_type, param->name, sizeof(_debug_buf), _debug_buf,
		     param->annotations.num);
	} else {
		blog(LOG_DEBUG, "%s[%4lld] %.*s '%s' with value %.*s.", offset, idx, sizeof(_debug_type), _debug_type,
		     param->name, sizeof(_debug_buf), _debug_buf);
	}
}

static void debug_param_annotation(struct gs_effect_param *param, struct ep_param *param_in, unsigned long long idx,
				   const char *offset)
{
	char _debug_buf[4096];
	debug_get_default_value(param, _debug_buf, sizeof(_debug_buf));
	blog(LOG_DEBUG, "%s[%4lld] %s '%s' with value %.*s", offset, idx, param_in->type, param->name,
	     sizeof(_debug_buf), _debug_buf);
}

void debug_print_string(const char *offset, const char *str)
{
	// Bypass 4096 limit in def_log_handler.
	char const *begin = str;
	unsigned long long line = 1;
	for (char const *here = begin; here[0] != '\0'; here++) {
		char const *str = begin;
		unsigned long long len = here - begin;
		bool is_line = false;

		if (here[0] == '\r') {
			is_line = true;
			if (here[1] == '\n') {
				here += 1;
			}
			begin = here + 1;
		} else if (here[0] == '\n') {
			is_line = true;
			begin = here + 1;
		}

		if (is_line) {
			blog(LOG_DEBUG, "\t\t\t\t[%4lld] %.*s", line, len, str);
			line++;
		}
	}
	if (begin[0] != '\0') {
		// Final line was not written.
		blog(LOG_DEBUG, "\t\t\t\t[%4lld] %*s", line, strlen(begin), begin);
	}
}
#endif

bool ep_parse(struct effect_parser *ep, gs_effect_t *effect, const char *effect_string, const char *file)
{
	bool success;

	const char *graphics_preprocessor = gs_preprocessor_name();

	if (graphics_preprocessor) {
		struct cf_def def;

		cf_def_init(&def);
		def.name.str.array = graphics_preprocessor;
		def.name.str.len = strlen(graphics_preprocessor);

		strref_copy(&def.name.unmerged_str, &def.name.str);
		cf_preprocessor_add_def(&ep->cfp.pp, &def);
	}

	ep->effect = effect;
	if (!cf_parser_parse(&ep->cfp, effect_string, file))
		return false;

	while (ep->cfp.cur_token && ep->cfp.cur_token->type != CFTOKEN_NONE) {
		if (cf_token_is(&ep->cfp, ";") || is_whitespace(*ep->cfp.cur_token->str.array)) {
			/* do nothing */
			ep->cfp.cur_token++;

		} else if (cf_token_is(&ep->cfp, "struct")) {
			ep_parse_struct(ep);

		} else if (cf_token_is(&ep->cfp, "technique")) {
			ep_parse_technique(ep);

		} else if (cf_token_is(&ep->cfp, "sampler_state")) {
			ep_parse_sampler_state(ep);

		} else if (cf_token_is(&ep->cfp, "{")) {
			/* add error and pass braces */
			cf_adderror(&ep->cfp, "Unexpected code segment", LEX_ERROR, NULL, NULL, NULL);
			cf_pass_pair(&ep->cfp, '{', '}');

		} else {
			/* parameters and functions */
			ep_parse_other(ep);
		}
	}

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
	blog(LOG_DEBUG, "================================================================================");
	blog(LOG_DEBUG, "Effect Parser reformatted shader '%s' to:", file);
	debug_print_string("\t", ep->cfp.lex.reformatted);
#endif

	success = !error_data_has_errors(&ep->cfp.error_list);
	if (success)
		success = ep_compile(ep);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
	blog(LOG_DEBUG, "================================================================================");
#endif

	return success;
}
