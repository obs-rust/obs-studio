/* Test-only oracle: the original libobs/graphics/effect-parser-compile.c.h,
 * unmodified, with ep_compile renamed to oracle_ep_compile so it can link
 * next to the Rust shim (its helpers are static). It runs on the C shader
 * writer of oracle/effect_parser_writer.c (oracle_ep_makeshaderstring), the
 * C parameter types of oracle/effect_param_types.c, the C effect param
 * lookup of oracle/effect_lookups.c, the C cf_adderror of oracle/decl.c and
 * the C dstr of oracle/dstr.c and oracle/dstr_libc.c, under their oracle_*
 * names, so the whole compile step is the original C.
 * The graphics calls go to the recording stand-in of
 * oracle/effect_parser_compile_device.c, as the shim's do, and blog() to the
 * log handler the test sets. bmalloc stays the real symbol: the Rust port
 * and this oracle share it.
 *
 * The helpers below set up an effect_parser as ep_parse() hands it to the
 * compile step (a parsed text, so cf_adderror has a token to point at, with
 * cur_token at the end), and free it and its effect, so the parity test can
 * fill the parser's params and techniques itself and give both sides the
 * same one. */
#define ep_compile oracle_ep_compile

#define ep_makeshaderstring oracle_ep_makeshaderstring
#define obs_effect_param_type oracle_obs_effect_param_type
#define gs_effect_get_param_by_name oracle_gs_effect_get_param_by_name
#define cf_adderror oracle_cf_adderror
#define dstr_copy oracle_dstr_copy
#define dstr_ncat oracle_dstr_ncat
#define dstr_catf oracle_dstr_catf
#define cf_preprocess oracle_cf_preprocess
#define cf_preprocessor_init oracle_cf_preprocessor_init
#define cf_preprocessor_free oracle_cf_preprocessor_free
#define cf_lexer_init oracle_cf_lexer_init
#define cf_lexer_free oracle_cf_lexer_free
#define cf_lexer_lex oracle_cf_lexer_lex

#include "graphics/effect-parser-compile.c.h"

bool oracle_effect_parser_compile_prepare(struct effect_parser *ep, gs_effect_t *effect, const char *file);
void oracle_effect_parser_compile_free(struct effect_parser *ep, gs_effect_t *effect);
void oracle_effect_parser_compile_effect_reset(gs_effect_t *effect);
size_t oracle_effect_parser_compile_layout(size_t *out);

/* ep_init(), the effect, then cf_parser_parse() of a short text, with
 * cur_token at its end token, where ep_parse() leaves it. */
bool oracle_effect_parser_compile_prepare(struct effect_parser *ep, gs_effect_t *effect, const char *file)
{
	ep_init(ep);
	effect_init(effect);
	ep->effect = effect;
	if (!cf_parser_parse(&ep->cfp, "uniform float4 x;\n", file))
		return false;
	while (ep->cfp.cur_token->type != CFTOKEN_NONE)
		ep->cfp.cur_token++;
	return true;
}

/* ep_free()'s params and techniques, the cf parser, then effect_free(). */
void oracle_effect_parser_compile_free(struct effect_parser *ep, gs_effect_t *effect)
{
	for (size_t i = 0; i < ep->params.num; i++)
		ep_param_free(ep->params.array + i);
	for (size_t i = 0; i < ep->techniques.num; i++)
		ep_technique_free(ep->techniques.array + i);
	da_free(ep->params);
	da_free(ep->structs);
	da_free(ep->funcs);
	da_free(ep->samplers);
	da_free(ep->techniques);
	cf_parser_free(&ep->cfp);
	effect_free(effect);
}

/* effect_free(), then effect_init(): for the bench, which compiles the same
 * parser again into the same effect. */
void oracle_effect_parser_compile_effect_reset(gs_effect_t *effect)
{
	effect_free(effect);
	effect_init(effect);
}

/* size, align, then the offset of every field, of struct pass_shaderparam,
 * ep_param and effect_parser in declaration order (with the size of enum
 * effect_section). Returns how many values it wrote. */
size_t oracle_effect_parser_compile_layout(size_t *out)
{
	size_t n = 0;

	out[n++] = sizeof(struct pass_shaderparam);
	out[n++] = _Alignof(struct pass_shaderparam);
	out[n++] = offsetof(struct pass_shaderparam, eparam);
	out[n++] = offsetof(struct pass_shaderparam, sparam);

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

	out[n++] = sizeof(enum effect_section);
	return n;
}
