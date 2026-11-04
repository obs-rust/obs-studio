/* Test-only oracle: the original libobs/graphics/effect-parser-blocks.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims. It runs on the original cf-parser.h inlines, with
 * the C cf_adderror, cf_pass_pair, lexer and preprocessor of oracle/decl.c,
 * the C strref_cmp of oracle/lexer.c and the C dstr_ncat of oracle/dstr.c,
 * under their oracle_* names, so the whole block is the original C. bmalloc
 * stays the real symbol: the Rust port and this oracle share it.
 *
 * The helpers below lex and preprocess a text with that same C
 * (cf_parser_parse()) into an effect_parser at a chosen token, and free it,
 * so the parity test gives both sides the same tokens. */
#define ep_parse_sampler_state oracle_ep_parse_sampler_state
#define ep_parse_struct oracle_ep_parse_struct

#define cf_adderror oracle_cf_adderror
#define cf_pass_pair oracle_cf_pass_pair
#define cf_preprocess oracle_cf_preprocess
#define cf_preprocessor_init oracle_cf_preprocessor_init
#define cf_preprocessor_free oracle_cf_preprocessor_free
#define cf_lexer_init oracle_cf_lexer_init
#define cf_lexer_free oracle_cf_lexer_free
#define cf_lexer_lex oracle_cf_lexer_lex
#define strref_cmp oracle_strref_cmp
#define dstr_ncat oracle_dstr_ncat

#include "graphics/effect-parser-blocks.c.h"

size_t oracle_effect_parser_blocks_prepare(struct effect_parser *ep, const char *text, size_t start);
size_t oracle_effect_parser_blocks_position(struct effect_parser *ep);
void oracle_effect_parser_blocks_clear(struct effect_parser *ep);
void oracle_effect_parser_blocks_free(struct effect_parser *ep);
size_t oracle_effect_parser_blocks_layout(size_t *out);

/* ep_init(), then cf_parser_parse() of text. Returns how many tokens it made
 * before the terminator (0 when the parse failed), with cur_token at token
 * start (the terminator when start is past the end). */
size_t oracle_effect_parser_blocks_prepare(struct effect_parser *ep, const char *text, size_t start)
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

/* The index of cur_token in the token array. */
size_t oracle_effect_parser_blocks_position(struct effect_parser *ep)
{
	return (size_t)(ep->cfp.cur_token - cf_preprocessor_get_tokens(&ep->cfp.pp));
}

/* Frees the structs and samplers pushed so far and the errors, as ep_free()
 * does, and keeps the tokens: for the bench, which runs the blocks again on
 * the same tokens. */
void oracle_effect_parser_blocks_clear(struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->structs.num; i++)
		ep_struct_free(ep->structs.array + i);
	for (size_t i = 0; i < ep->samplers.num; i++)
		ep_sampler_free(ep->samplers.array + i);
	ep->structs.num = 0;
	ep->samplers.num = 0;
	error_data_free(&ep->cfp.error_list);
	error_data_init(&ep->cfp.error_list);
}

/* What ep_free() frees of a parser only these blocks wrote to: the structs
 * and samplers with their storage, and the cf parser. */
void oracle_effect_parser_blocks_free(struct effect_parser *ep)
{
	oracle_effect_parser_blocks_clear(ep);
	da_free(ep->structs);
	da_free(ep->samplers);
	cf_parser_free(&ep->cfp);
}

/* size, align, then the offset of every field, of struct ep_var, ep_struct,
 * ep_sampler and effect_parser in declaration order. Returns how many values
 * it wrote. */
size_t oracle_effect_parser_blocks_layout(size_t *out)
{
	size_t n = 0;

	out[n++] = sizeof(struct ep_var);
	out[n++] = _Alignof(struct ep_var);
	out[n++] = offsetof(struct ep_var, type);
	out[n++] = offsetof(struct ep_var, name);
	out[n++] = offsetof(struct ep_var, mapping);
	out[n++] = offsetof(struct ep_var, var_type);
	out[n++] = sizeof(enum ep_var_type);

	out[n++] = sizeof(struct ep_struct);
	out[n++] = _Alignof(struct ep_struct);
	out[n++] = offsetof(struct ep_struct, name);
	out[n++] = offsetof(struct ep_struct, vars);
	out[n++] = offsetof(struct ep_struct, written);

	out[n++] = sizeof(struct ep_sampler);
	out[n++] = _Alignof(struct ep_sampler);
	out[n++] = offsetof(struct ep_sampler, name);
	out[n++] = offsetof(struct ep_sampler, states);
	out[n++] = offsetof(struct ep_sampler, values);
	out[n++] = offsetof(struct ep_sampler, written);

	out[n++] = sizeof(struct effect_parser);
	out[n++] = _Alignof(struct effect_parser);
	out[n++] = offsetof(struct effect_parser, effect);
	out[n++] = offsetof(struct effect_parser, params);
	out[n++] = offsetof(struct effect_parser, structs);
	out[n++] = offsetof(struct effect_parser, funcs);
	out[n++] = offsetof(struct effect_parser, samplers);
	out[n++] = offsetof(struct effect_parser, techniques);
	out[n++] = offsetof(struct effect_parser, files);
	out[n++] = offsetof(struct effect_parser, tokens);
	out[n++] = offsetof(struct effect_parser, cur_pass);
	out[n++] = offsetof(struct effect_parser, cfp);
	return n;
}
