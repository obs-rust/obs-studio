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

#include "libobs/build/libobs-shader-parser-blocks.pch.h"

/* ------------------------------------------------------------------------- */

static int sp_parse_sampler_state_item(struct shader_parser *sp, struct shader_sampler *ss)
{
	int ret;
	char *state = NULL, *value = NULL;

	ret = cf_next_name(&sp->cfp, &state, "state name", ";");
	if (ret != PARSE_SUCCESS)
		goto fail;

	ret = cf_next_token_should_be(&sp->cfp, "=", ";", NULL);
	if (ret != PARSE_SUCCESS)
		goto fail;

	ret = cf_next_token_copy(&sp->cfp, &value);
	if (ret != PARSE_SUCCESS)
		goto fail;

	ret = cf_next_token_should_be(&sp->cfp, ";", ";", NULL);
	if (ret != PARSE_SUCCESS)
		goto fail;

	da_push_back(ss->states, &state);
	da_push_back(ss->values, &value);
	return ret;

fail:
	bfree(state);
	bfree(value);
	return ret;
}

void sp_parse_sampler_state(struct shader_parser *sp)
{
	struct shader_sampler ss;
	struct cf_token peek;
	shader_sampler_init(&ss);

	if (cf_next_name(&sp->cfp, &ss.name, "name", ";") != PARSE_SUCCESS)
		goto error;
	if (cf_next_token_should_be(&sp->cfp, "{", ";", NULL) != PARSE_SUCCESS)
		goto error;

	if (!cf_peek_valid_token(&sp->cfp, &peek))
		goto error;

	while (strref_cmp(&peek.str, "}") != 0) {
		int ret = sp_parse_sampler_state_item(sp, &ss);
		if (ret == PARSE_EOF)
			goto error;

		if (!cf_peek_valid_token(&sp->cfp, &peek))
			goto error;
	}

	if (cf_next_token_should_be(&sp->cfp, "}", ";", NULL) != PARSE_SUCCESS)
		goto error;
	if (cf_next_token_should_be(&sp->cfp, ";", NULL, NULL) != PARSE_SUCCESS)
		goto error;

	da_push_back(sp->samplers, &ss);
	return;

error:
	shader_sampler_free(&ss);
}

static inline int sp_parse_struct_var(struct shader_parser *sp, struct shader_var *var)
{
	int code;

	/* -------------------------------------- */
	/* variable type */

	if (!cf_next_valid_token(&sp->cfp))
		return PARSE_EOF;

	if (cf_token_is(&sp->cfp, ";"))
		return PARSE_CONTINUE;
	if (cf_token_is(&sp->cfp, "}"))
		return PARSE_BREAK;

	code = cf_token_is_type(&sp->cfp, CFTOKEN_NAME, "type name", ";");
	if (code != PARSE_SUCCESS)
		return code;

	cf_copy_token(&sp->cfp, &var->type);

	/* -------------------------------------- */
	/* variable name */

	if (!cf_next_valid_token(&sp->cfp))
		return PARSE_EOF;

	if (cf_token_is(&sp->cfp, ";"))
		return PARSE_UNEXPECTED_CONTINUE;
	if (cf_token_is(&sp->cfp, "}"))
		return PARSE_UNEXPECTED_BREAK;

	code = cf_token_is_type(&sp->cfp, CFTOKEN_NAME, "variable name", ";");
	if (code != PARSE_SUCCESS)
		return code;

	cf_copy_token(&sp->cfp, &var->name);

	/* -------------------------------------- */
	/* variable mapping if any (POSITION, TEXCOORD, etc) */

	if (!cf_next_valid_token(&sp->cfp))
		return PARSE_EOF;

	if (cf_token_is(&sp->cfp, ":")) {
		if (!cf_next_valid_token(&sp->cfp))
			return PARSE_EOF;

		if (cf_token_is(&sp->cfp, ";"))
			return PARSE_UNEXPECTED_CONTINUE;
		if (cf_token_is(&sp->cfp, "}"))
			return PARSE_UNEXPECTED_BREAK;

		code = cf_token_is_type(&sp->cfp, CFTOKEN_NAME, "mapping name", ";");
		if (code != PARSE_SUCCESS)
			return code;

		cf_copy_token(&sp->cfp, &var->mapping);

		if (!cf_next_valid_token(&sp->cfp))
			return PARSE_EOF;
	}

	/* -------------------------------------- */

	if (!cf_token_is(&sp->cfp, ";")) {
		if (!cf_go_to_valid_token(&sp->cfp, ";", "}"))
			return PARSE_EOF;
		return PARSE_CONTINUE;
	}

	return PARSE_SUCCESS;
}

void sp_parse_struct(struct shader_parser *sp)
{
	struct shader_struct ss;
	shader_struct_init(&ss);

	if (cf_next_name(&sp->cfp, &ss.name, "name", ";") != PARSE_SUCCESS)
		goto error;
	if (cf_next_token_should_be(&sp->cfp, "{", ";", NULL) != PARSE_SUCCESS)
		goto error;

	/* get structure variables */
	while (true) {
		bool do_break = false;
		struct shader_var var;

		shader_var_init(&var);

		switch (sp_parse_struct_var(sp, &var)) {

		case PARSE_UNEXPECTED_CONTINUE:
			cf_adderror_syntax_error(&sp->cfp);
			/* Falls through. */
		case PARSE_CONTINUE:
			shader_var_free(&var);
			continue;

		case PARSE_UNEXPECTED_BREAK:
			cf_adderror_syntax_error(&sp->cfp);
			/* Falls through. */
		case PARSE_BREAK:
			shader_var_free(&var);
			do_break = true;
			break;

		case PARSE_EOF:
			shader_var_free(&var);
			goto error;
		}

		if (do_break)
			break;

		da_push_back(ss.vars, &var);
	}

	if (cf_next_token_should_be(&sp->cfp, ";", NULL, NULL) != PARSE_SUCCESS)
		goto error;

	da_push_back(sp->structs, &ss);
	return;

error:
	shader_struct_free(&ss);
}
