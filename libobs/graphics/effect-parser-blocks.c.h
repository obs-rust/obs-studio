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

#include "libobs/build/libobs-effect-parser-blocks.pch.h"

/* ------------------------------------------------------------------------- */

static inline int ep_parse_struct_var(struct effect_parser *ep, struct ep_var *var)
{
	int code;

	/* -------------------------------------- */
	/* variable type */

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, ";"))
		return PARSE_CONTINUE;
	if (cf_token_is(&ep->cfp, "}"))
		return PARSE_BREAK;

	code = cf_token_is_type(&ep->cfp, CFTOKEN_NAME, "type name", ";");
	if (code != PARSE_SUCCESS)
		return code;

	cf_copy_token(&ep->cfp, &var->type);

	/* -------------------------------------- */
	/* variable name */

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, ";"))
		return PARSE_UNEXPECTED_CONTINUE;
	if (cf_token_is(&ep->cfp, "}"))
		return PARSE_UNEXPECTED_BREAK;

	code = cf_token_is_type(&ep->cfp, CFTOKEN_NAME, "variable name", ";");
	if (code != PARSE_SUCCESS)
		return code;

	cf_copy_token(&ep->cfp, &var->name);

	/* -------------------------------------- */
	/* variable mapping if any (POSITION, TEXCOORD, etc) */

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, ":")) {
		if (!cf_next_valid_token(&ep->cfp))
			return PARSE_EOF;

		if (cf_token_is(&ep->cfp, ";"))
			return PARSE_UNEXPECTED_CONTINUE;
		if (cf_token_is(&ep->cfp, "}"))
			return PARSE_UNEXPECTED_BREAK;

		code = cf_token_is_type(&ep->cfp, CFTOKEN_NAME, "mapping name", ";");
		if (code != PARSE_SUCCESS)
			return code;

		cf_copy_token(&ep->cfp, &var->mapping);

		if (!cf_next_valid_token(&ep->cfp))
			return PARSE_EOF;
	}

	/* -------------------------------------- */

	if (!cf_token_is(&ep->cfp, ";")) {
		if (!cf_go_to_valid_token(&ep->cfp, ";", "}"))
			return PARSE_EOF;
		return PARSE_CONTINUE;
	}

	return PARSE_SUCCESS;
}

void ep_parse_struct(struct effect_parser *ep)
{
	struct ep_struct eps;
	ep_struct_init(&eps);

	if (cf_next_name(&ep->cfp, &eps.name, "name", ";") != PARSE_SUCCESS)
		goto error;
	if (cf_next_token_should_be(&ep->cfp, "{", ";", NULL) != PARSE_SUCCESS)
		goto error;

	/* get structure variables */
	while (true) {
		bool do_break = false;
		struct ep_var var;

		ep_var_init(&var);

		switch (ep_parse_struct_var(ep, &var)) {

		case PARSE_UNEXPECTED_CONTINUE:
			cf_adderror_syntax_error(&ep->cfp);
			/* Falls through. */
		case PARSE_CONTINUE:
			ep_var_free(&var);
			continue;

		case PARSE_UNEXPECTED_BREAK:
			cf_adderror_syntax_error(&ep->cfp);
			/* Falls through. */
		case PARSE_BREAK:
			ep_var_free(&var);
			do_break = true;
			break;

		case PARSE_EOF:
			ep_var_free(&var);
			goto error;
		}

		if (do_break)
			break;

		da_push_back(eps.vars, &var);
	}

	if (cf_next_token_should_be(&ep->cfp, ";", NULL, NULL) != PARSE_SUCCESS)
		goto error;

	da_push_back(ep->structs, &eps);
	return;

error:
	ep_struct_free(&eps);
}

static int ep_parse_sampler_state_item(struct effect_parser *ep, struct ep_sampler *eps)
{
	int ret;
	char *state = NULL;
	struct dstr value = {0};

	ret = cf_next_name(&ep->cfp, &state, "state name", ";");
	if (ret != PARSE_SUCCESS)
		goto fail;

	ret = cf_next_token_should_be(&ep->cfp, "=", ";", NULL);
	if (ret != PARSE_SUCCESS)
		goto fail;

	for (;;) {
		const char *cur_str;

		if (!cf_next_valid_token(&ep->cfp)) {
			ret = PARSE_EOF;
			goto fail;
		}

		cur_str = ep->cfp.cur_token->str.array;
		if (*cur_str == ';')
			break;

		dstr_ncat(&value, cur_str, ep->cfp.cur_token->str.len);
	}

	if (value.len) {
		da_push_back(eps->states, &state);
		da_push_back(eps->values, &value.array);
	} else {
		bfree(state);
		dstr_free(&value);
	}

	return ret;

fail:
	bfree(state);
	dstr_free(&value);
	return ret;
}

void ep_parse_sampler_state(struct effect_parser *ep)
{
	struct ep_sampler eps;
	struct cf_token peek;
	ep_sampler_init(&eps);

	if (cf_next_name(&ep->cfp, &eps.name, "name", ";") != PARSE_SUCCESS)
		goto error;
	if (cf_next_token_should_be(&ep->cfp, "{", ";", NULL) != PARSE_SUCCESS)
		goto error;

	if (!cf_peek_valid_token(&ep->cfp, &peek))
		goto error;

	while (strref_cmp(&peek.str, "}") != 0) {
		int ret = ep_parse_sampler_state_item(ep, &eps);
		if (ret == PARSE_EOF)
			goto error;

		if (!cf_peek_valid_token(&ep->cfp, &peek))
			goto error;
	}

	if (cf_next_token_should_be(&ep->cfp, "}", ";", NULL) != PARSE_SUCCESS)
		goto error;
	if (cf_next_token_should_be(&ep->cfp, ";", NULL, NULL) != PARSE_SUCCESS)
		goto error;

	da_push_back(ep->samplers, &eps);
	return;

error:
	ep_sampler_free(&eps);
}
