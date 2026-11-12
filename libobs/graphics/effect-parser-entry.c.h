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
