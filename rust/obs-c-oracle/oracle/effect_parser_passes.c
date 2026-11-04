/* Test-only oracle: the original libobs/graphics/effect-parser-passes.c.h,
 * unmodified, with ep_parse_technique renamed to oracle_ep_parse_technique so
 * it can link next to the Rust shim (its pass and command helpers are
 * static). It runs on the original cf-parser.h inlines, with the C
 * cf_adderror, cf_pass_pair, lexer and preprocessor of oracle/decl.c and the
 * C strref_cmp of oracle/lexer.c, under their oracle_* names, so the whole
 * technique is the original C. bmalloc stays the real symbol: the Rust port
 * and this oracle share it.
 *
 * The helpers below lex and preprocess a text with that same C
 * (cf_parser_parse()) into an effect_parser at a chosen token, and free it,
 * so the parity test gives both sides the same tokens. */
#define ep_parse_technique oracle_ep_parse_technique

#define cf_adderror oracle_cf_adderror
#define cf_pass_pair oracle_cf_pass_pair
#define cf_preprocess oracle_cf_preprocess
#define cf_preprocessor_init oracle_cf_preprocessor_init
#define cf_preprocessor_free oracle_cf_preprocessor_free
#define cf_lexer_init oracle_cf_lexer_init
#define cf_lexer_free oracle_cf_lexer_free
#define cf_lexer_lex oracle_cf_lexer_lex
#define strref_cmp oracle_strref_cmp

#include "graphics/effect-parser-passes.c.h"

size_t oracle_effect_parser_passes_prepare(struct effect_parser *ep, const char *text, size_t start);
struct cf_token *oracle_effect_parser_passes_tokens(struct effect_parser *ep);
void oracle_effect_parser_passes_clear(struct effect_parser *ep);
void oracle_effect_parser_passes_free(struct effect_parser *ep);
size_t oracle_effect_parser_passes_layout(size_t *out);

/* ep_init(), then cf_parser_parse() of text. Returns how many tokens it made
 * before the terminator (0 when the parse failed), with cur_token at token
 * start (the terminator when start is past the end). */
size_t oracle_effect_parser_passes_prepare(struct effect_parser *ep, const char *text, size_t start)
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
struct cf_token *oracle_effect_parser_passes_tokens(struct effect_parser *ep)
{
	return cf_preprocessor_get_tokens(&ep->cfp.pp);
}

/* Frees the techniques pushed so far and the errors, as ep_free() does, and
 * keeps the tokens: for the bench, which runs the technique again on the same
 * tokens. */
void oracle_effect_parser_passes_clear(struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->techniques.num; i++)
		ep_technique_free(ep->techniques.array + i);
	ep->techniques.num = 0;
	error_data_free(&ep->cfp.error_list);
	error_data_init(&ep->cfp.error_list);
}

/* What ep_free() frees of a parser only the techniques wrote to: the
 * techniques with their storage, and the cf parser. */
void oracle_effect_parser_passes_free(struct effect_parser *ep)
{
	oracle_effect_parser_passes_clear(ep);
	da_free(ep->techniques);
	cf_parser_free(&ep->cfp);
}

/* size, align, then the offset of every field, of struct ep_pass,
 * ep_technique and cf_token in declaration order. Returns how many values it
 * wrote. */
size_t oracle_effect_parser_passes_layout(size_t *out)
{
	size_t n = 0;

	out[n++] = sizeof(struct ep_pass);
	out[n++] = _Alignof(struct ep_pass);
	out[n++] = offsetof(struct ep_pass, name);
	out[n++] = offsetof(struct ep_pass, vertex_program);
	out[n++] = offsetof(struct ep_pass, fragment_program);
	out[n++] = offsetof(struct ep_pass, pass);

	out[n++] = sizeof(struct ep_technique);
	out[n++] = _Alignof(struct ep_technique);
	out[n++] = offsetof(struct ep_technique, name);
	out[n++] = offsetof(struct ep_technique, passes);

	out[n++] = sizeof(struct cf_token);
	out[n++] = _Alignof(struct cf_token);
	out[n++] = offsetof(struct cf_token, lex);
	out[n++] = offsetof(struct cf_token, str);
	out[n++] = offsetof(struct cf_token, unmerged_str);
	out[n++] = offsetof(struct cf_token, type);
	out[n++] = sizeof(enum cf_token_type);
	return n;
}
