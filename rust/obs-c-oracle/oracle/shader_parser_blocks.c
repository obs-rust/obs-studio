/* Test-only oracle: the original libobs/graphics/shader-parser-blocks.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims. It runs on the original cf-parser.h inlines, with
 * the C cf_adderror, cf_pass_pair, lexer and preprocessor of oracle/decl.c
 * and the C strref_cmp of oracle/lexer.c, under their oracle_* names, so the
 * whole block is the original C. bmalloc stays the real symbol: the Rust
 * port and this oracle share it.
 *
 * The helpers below lex and preprocess a text with that same C
 * (cf_parser_parse()) into a shader_parser at a chosen token, and free it,
 * so the parity test gives both sides the same tokens. */
#define sp_parse_sampler_state oracle_sp_parse_sampler_state
#define sp_parse_struct oracle_sp_parse_struct

#define cf_adderror oracle_cf_adderror
#define cf_pass_pair oracle_cf_pass_pair
#define cf_preprocess oracle_cf_preprocess
#define cf_preprocessor_init oracle_cf_preprocessor_init
#define cf_preprocessor_free oracle_cf_preprocessor_free
#define cf_lexer_init oracle_cf_lexer_init
#define cf_lexer_free oracle_cf_lexer_free
#define cf_lexer_lex oracle_cf_lexer_lex
#define strref_cmp oracle_strref_cmp

#include "graphics/shader-parser-blocks.c.h"

size_t oracle_shader_parser_blocks_prepare(struct shader_parser *sp, const char *text, size_t start);
size_t oracle_shader_parser_blocks_position(struct shader_parser *sp);
void oracle_shader_parser_blocks_free(struct shader_parser *sp);
size_t oracle_shader_parser_blocks_layout(size_t *out);

/* shader_parser_init(), then cf_parser_parse() of text. Returns how many
 * tokens it made before the terminator (0 when the parse failed), with
 * cur_token at token start (the terminator when start is past the end). */
size_t oracle_shader_parser_blocks_prepare(struct shader_parser *sp, const char *text, size_t start)
{
	size_t count = 0;

	shader_parser_init(sp);
	if (!cf_parser_parse(&sp->cfp, text, "parity"))
		return 0;
	while (sp->cfp.cur_token[count].type != CFTOKEN_NONE)
		count++;
	sp->cfp.cur_token += start < count ? start : count;
	return count;
}

/* The index of cur_token in the token array. */
size_t oracle_shader_parser_blocks_position(struct shader_parser *sp)
{
	return (size_t)(sp->cfp.cur_token - cf_preprocessor_get_tokens(&sp->cfp.pp));
}

void oracle_shader_parser_blocks_free(struct shader_parser *sp)
{
	shader_parser_free(sp);
}

/* size, align, then the offset of every field, of struct shader_var,
 * shader_struct, shader_sampler and shader_parser in declaration order.
 * Returns how many values it wrote. */
size_t oracle_shader_parser_blocks_layout(size_t *out)
{
	size_t n = 0;

	out[n++] = sizeof(struct shader_var);
	out[n++] = _Alignof(struct shader_var);
	out[n++] = offsetof(struct shader_var, type);
	out[n++] = offsetof(struct shader_var, name);
	out[n++] = offsetof(struct shader_var, mapping);
	out[n++] = offsetof(struct shader_var, var_type);
	out[n++] = offsetof(struct shader_var, array_count);
	out[n++] = offsetof(struct shader_var, gl_sampler_id);
	out[n++] = offsetof(struct shader_var, default_val);

	out[n++] = sizeof(struct shader_struct);
	out[n++] = _Alignof(struct shader_struct);
	out[n++] = offsetof(struct shader_struct, name);
	out[n++] = offsetof(struct shader_struct, vars);

	out[n++] = sizeof(struct shader_sampler);
	out[n++] = _Alignof(struct shader_sampler);
	out[n++] = offsetof(struct shader_sampler, name);
	out[n++] = offsetof(struct shader_sampler, states);
	out[n++] = offsetof(struct shader_sampler, values);

	out[n++] = sizeof(struct shader_parser);
	out[n++] = _Alignof(struct shader_parser);
	out[n++] = offsetof(struct shader_parser, cfp);
	out[n++] = offsetof(struct shader_parser, params);
	out[n++] = offsetof(struct shader_parser, structs);
	out[n++] = offsetof(struct shader_parser, samplers);
	out[n++] = offsetof(struct shader_parser, funcs);

	out[n++] = sizeof(struct cf_token);
	out[n++] = _Alignof(struct cf_token);
	out[n++] = offsetof(struct cf_token, lex);
	out[n++] = offsetof(struct cf_token, str);
	out[n++] = offsetof(struct cf_token, unmerged_str);
	out[n++] = offsetof(struct cf_token, type);
	return n;
}

/* Frees the samplers and structs pushed so far and the errors, as
 * shader_parser_free() does, and keeps the tokens: for the bench, which
 * runs the blocks again on the same tokens. */
void oracle_shader_parser_blocks_clear(struct shader_parser *sp);

void oracle_shader_parser_blocks_clear(struct shader_parser *sp)
{
	for (size_t i = 0; i < sp->structs.num; i++)
		shader_struct_free(sp->structs.array + i);
	for (size_t i = 0; i < sp->samplers.num; i++)
		shader_sampler_free(sp->samplers.array + i);
	sp->structs.num = 0;
	sp->samplers.num = 0;
	error_data_free(&sp->cfp.error_list);
	error_data_init(&sp->cfp.error_list);
}
