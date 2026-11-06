/* Test-only oracle: the original libobs/graphics/effect-parser-params.c.h,
 * unmodified, with ep_parse_param, ep_parse_param_array and
 * ep_parse_param_assign renamed to oracle_* so it can link next to the Rust
 * shim (the other helpers are static). It runs on the original cf-parser.h
 * inlines, with the C cf_adderror, cf_pass_pair, lexer, preprocessor and
 * cf_literal_to_str of oracle/decl.c, the C strref_cmp of oracle/lexer.c, the
 * C parameter type decisions and number reads of
 * oracle/effect_param_types.c and oracle/effect_param_numbers.c, the C
 * os_strtod of oracle/platform_number.c and the C astrcmp_n of
 * oracle/dstr.c, under their oracle_* names, so the whole declaration is the
 * original C. bmalloc stays the real symbol: the Rust port and this oracle
 * share it.
 *
 * The helpers below lex and preprocess a text with that same C
 * (cf_parser_parse()) into an effect_parser at a chosen token, copy a type
 * or name as ep_parse_other() hands it over, and free the parser, so the
 * parity test gives both sides the same tokens. */
#define ep_parse_param oracle_ep_parse_param
#define ep_parse_param_array oracle_ep_parse_param_array
#define ep_parse_param_assign oracle_ep_parse_param_assign

#define cf_adderror oracle_cf_adderror
#define cf_pass_pair oracle_cf_pass_pair
#define cf_preprocess oracle_cf_preprocess
#define cf_preprocessor_init oracle_cf_preprocessor_init
#define cf_preprocessor_free oracle_cf_preprocessor_free
#define cf_lexer_init oracle_cf_lexer_init
#define cf_lexer_free oracle_cf_lexer_free
#define cf_lexer_lex oracle_cf_lexer_lex
#define cf_literal_to_str oracle_cf_literal_to_str
#define strref_cmp oracle_strref_cmp
#define astrcmp_n oracle_astrcmp_n
#define os_strtod oracle_os_strtod

#define obs_effect_param_type oracle_obs_effect_param_type
#define obs_effect_param_route(...) oracle_obs_effect_param_route(__VA_ARGS__)
#define obs_effect_param_rows_bad oracle_obs_effect_param_rows_bad
#define obs_effect_param_rows oracle_obs_effect_param_rows
#define obs_effect_param_has_cols oracle_obs_effect_param_has_cols
#define obs_effect_param_cols_bad oracle_obs_effect_param_cols_bad
#define obs_effect_param_cols oracle_obs_effect_param_cols
#define obs_effect_param_int_value oracle_obs_effect_param_int_value
#define obs_effect_param_array_count oracle_obs_effect_param_array_count

#include "graphics/effect-parser-params.c.h"

size_t oracle_effect_parser_params_prepare(struct effect_parser *ep, const char *text, size_t start);
struct cf_token *oracle_effect_parser_params_tokens(struct effect_parser *ep);
char *oracle_effect_parser_params_strdup(const char *str);
void oracle_effect_parser_params_clear(struct effect_parser *ep);
void oracle_effect_parser_params_free(struct effect_parser *ep);
size_t oracle_effect_parser_params_layout(size_t *out);

/* ep_init(), then cf_parser_parse() of text. Returns how many tokens it made
 * before the terminator (0 when the parse failed), with cur_token at token
 * start (the terminator when start is past the end). */
size_t oracle_effect_parser_params_prepare(struct effect_parser *ep, const char *text, size_t start)
{
	size_t count = 0;

	ep_init(ep);
	if (!cf_parser_parse(&ep->cfp, text, "parity"))
		return 0;
	while (ep->cfp.cur_token[count].type != CFTOKEN_NONE)
		count++;
	ep->cfp.cur_token += start < count ? start : count;
	return count;
}

/* The parser's token array: the token cur_token walks, from its first. */
struct cf_token *oracle_effect_parser_params_tokens(struct effect_parser *ep)
{
	return cf_preprocessor_get_tokens(&ep->cfp.pp);
}

/* A type or name as ep_parse_other() allocates it (cf_get_name's
 * bstrdup_n), for ep_parse_param to own. */
char *oracle_effect_parser_params_strdup(const char *str)
{
	return bstrdup(str);
}

/* Frees the params pushed so far and the errors, as ep_free() does, and
 * keeps the tokens: for the bench, which runs the declaration again on the
 * same tokens. */
void oracle_effect_parser_params_clear(struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->params.num; i++)
		ep_param_free(ep->params.array + i);
	ep->params.num = 0;
	error_data_free(&ep->cfp.error_list);
	error_data_init(&ep->cfp.error_list);
}

/* What ep_free() frees of a parser only the declarations wrote to: the
 * params with their storage, and the cf parser. */
void oracle_effect_parser_params_free(struct effect_parser *ep)
{
	oracle_effect_parser_params_clear(ep);
	da_free(ep->params);
	cf_parser_free(&ep->cfp);
}

/* size, align, then the offset of every field of struct ep_param in
 * declaration order. Returns how many values it wrote. */
size_t oracle_effect_parser_params_layout(size_t *out)
{
	size_t n = 0;

	out[n++] = sizeof(struct ep_param);
	out[n++] = _Alignof(struct ep_param);
	out[n++] = offsetof(struct ep_param, type);
	out[n++] = offsetof(struct ep_param, name);
	out[n++] = offsetof(struct ep_param, default_val);
	out[n++] = offsetof(struct ep_param, properties);
	out[n++] = offsetof(struct ep_param, param);
	out[n++] = offsetof(struct ep_param, is_const);
	out[n++] = offsetof(struct ep_param, is_property);
	out[n++] = offsetof(struct ep_param, is_uniform);
	out[n++] = offsetof(struct ep_param, is_texture);
	out[n++] = offsetof(struct ep_param, written);
	out[n++] = offsetof(struct ep_param, writeorder);
	out[n++] = offsetof(struct ep_param, array_count);
	out[n++] = offsetof(struct ep_param, annotations);
	return n;
}
