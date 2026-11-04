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

#include "libobs/build/libobs-effect-parser-passes.pch.h"

/* ------------------------------------------------------------------------- */

static inline int ep_parse_pass_command_call(struct effect_parser *ep, cf_token_array_t *call)
{
	struct cf_token end_token;
	cf_token_clear(&end_token);

	while (!cf_token_is(&ep->cfp, ";")) {
		if (cf_token_is(&ep->cfp, "}")) {
			cf_adderror_expecting(&ep->cfp, ";");
			return PARSE_CONTINUE;
		}

		da_push_back(*call, ep->cfp.cur_token);
		if (!cf_next_valid_token(&ep->cfp))
			return PARSE_EOF;
	}

	da_push_back(*call, ep->cfp.cur_token);
	da_push_back(*call, &end_token);
	return PARSE_SUCCESS;
}

static int ep_parse_pass_command(struct effect_parser *ep, struct ep_pass *pass)
{
	cf_token_array_t *call;

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (cf_token_is(&ep->cfp, "vertex_shader") || cf_token_is(&ep->cfp, "vertex_program")) {
		call = &pass->vertex_program;

	} else if (cf_token_is(&ep->cfp, "pixel_shader") || cf_token_is(&ep->cfp, "pixel_program")) {
		call = &pass->fragment_program;

	} else {
		cf_adderror_syntax_error(&ep->cfp);
		if (!cf_go_to_valid_token(&ep->cfp, ";", "}"))
			return PARSE_EOF;
		return PARSE_CONTINUE;
	}

	if (cf_next_token_should_be(&ep->cfp, "=", ";", "}") != PARSE_SUCCESS)
		return PARSE_CONTINUE;

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;
	if (cf_token_is(&ep->cfp, "compile")) {
		cf_adderror(&ep->cfp, "compile keyword not necessary", LEX_WARNING, NULL, NULL, NULL);
		if (!cf_next_valid_token(&ep->cfp))
			return PARSE_EOF;
	}

	return ep_parse_pass_command_call(ep, call);
}

static int ep_parse_pass(struct effect_parser *ep, struct ep_pass *pass)
{
	struct cf_token peek;

	if (!cf_token_is(&ep->cfp, "pass"))
		return PARSE_UNEXPECTED_CONTINUE;

	if (!cf_next_valid_token(&ep->cfp))
		return PARSE_EOF;

	if (!cf_token_is(&ep->cfp, "{")) {
		pass->name = bstrdup_n(ep->cfp.cur_token->str.array, ep->cfp.cur_token->str.len);
		if (!cf_next_valid_token(&ep->cfp))
			return PARSE_EOF;
	}

	if (!cf_peek_valid_token(&ep->cfp, &peek))
		return PARSE_EOF;

	while (strref_cmp(&peek.str, "}") != 0) {
		int ret = ep_parse_pass_command(ep, pass);
		if (ret < 0 && ret != PARSE_CONTINUE)
			return ret;

		if (!cf_peek_valid_token(&ep->cfp, &peek))
			return PARSE_EOF;
	}

	/* token is '}' */
	cf_next_token(&ep->cfp);

	return PARSE_SUCCESS;
}

void ep_parse_technique(struct effect_parser *ep)
{
	struct ep_technique ept;
	ep_technique_init(&ept);

	if (cf_next_name(&ep->cfp, &ept.name, "name", ";") != PARSE_SUCCESS)
		goto error;

	if (!cf_next_valid_token(&ep->cfp)) {
		ep_technique_free(&ept);
		return;
	}

	if (!cf_token_is(&ep->cfp, "{")) {
		if (!cf_go_to_token(&ep->cfp, ";", NULL)) {
			cf_adderror_expecting(&ep->cfp, ";");
			ep_technique_free(&ept);
			return;
		}

		cf_adderror_expecting(&ep->cfp, "{");
		goto error;
	}
	if (!cf_next_valid_token(&ep->cfp))
		goto error;

	while (!cf_token_is(&ep->cfp, "}")) {
		struct ep_pass pass;
		ep_pass_init(&pass);

		switch (ep_parse_pass(ep, &pass)) {
		case PARSE_UNEXPECTED_CONTINUE:
			ep_pass_free(&pass);
			if (!cf_go_to_token(&ep->cfp, "}", NULL))
				goto error;
			continue;
		case PARSE_EOF:
			ep_pass_free(&pass);
			goto error;
		}

		da_push_back(ept.passes, &pass);

		if (!cf_next_valid_token(&ep->cfp))
			goto error;
	}

	/* pass the current token (which is '}') if we reached here */
	cf_next_token(&ep->cfp);

	da_push_back(ep->techniques, &ept);
	return;

error:
	cf_next_token(&ep->cfp);
	ep_technique_free(&ept);
}
