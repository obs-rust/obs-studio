/* Test-only oracle: the original libobs/graphics/effect-parser-writer.c.h,
 * unmodified, with ep_makeshaderstring renamed to oracle_ep_makeshaderstring
 * so it can link next to the Rust shim (the other writers and its two
 * lookups are static). The lookups it takes from the function parser
 * (ep_getstruct, ep_getsampler, ep_getfunc_strref and ep_getparam_strref)
 * are the original libobs/graphics/effect-parser-funcs.c.h, compiled here
 * under oracle_* names with the rest of that file (the function parser,
 * which nothing here calls). Both run on the C strref_cmp of oracle/lexer.c,
 * the C dstr of oracle/dstr.c and oracle/dstr_libc.c and the C cf_adderror
 * of oracle/decl.c, under their oracle_* names, so the whole writer is the
 * original C. bmalloc stays the real symbol: the Rust port and this oracle
 * share it. */
#define ep_makeshaderstring oracle_ep_makeshaderstring
#define ep_getstruct oracle_writer_ep_getstruct
#define ep_getsampler oracle_writer_ep_getsampler
#define ep_getfunc_strref oracle_writer_ep_getfunc_strref
#define ep_getparam_strref oracle_writer_ep_getparam_strref
#define ep_check_for_keyword oracle_writer_ep_check_for_keyword
#define ep_parse_function oracle_writer_ep_parse_function

#define cf_adderror oracle_cf_adderror
#define strref_cmp oracle_strref_cmp
#define dstr_copy oracle_dstr_copy
#define dstr_ncat oracle_dstr_ncat
#define dstr_cat_dstr oracle_dstr_cat_dstr
#define dstr_cat_strref oracle_dstr_cat_strref
#define dstr_catf oracle_dstr_catf

#include "graphics/effect-parser-funcs.c.h"
#include "graphics/effect-parser-writer.c.h"

size_t oracle_effect_parser_writer_layout(size_t *out);

/* size, align, then the offset of every field, of struct ep_func, ep_param,
 * ep_var, ep_struct, ep_sampler and dstr in declaration order (with the size
 * of enum ep_var_type). Returns how many values it wrote. */
size_t oracle_effect_parser_writer_layout(size_t *out)
{
	size_t n = 0;

	out[n++] = sizeof(struct ep_func);
	out[n++] = _Alignof(struct ep_func);
	out[n++] = offsetof(struct ep_func, name);
	out[n++] = offsetof(struct ep_func, ret_type);
	out[n++] = offsetof(struct ep_func, mapping);
	out[n++] = offsetof(struct ep_func, contents);
	out[n++] = offsetof(struct ep_func, param_vars);
	out[n++] = offsetof(struct ep_func, func_deps);
	out[n++] = offsetof(struct ep_func, struct_deps);
	out[n++] = offsetof(struct ep_func, param_deps);
	out[n++] = offsetof(struct ep_func, sampler_deps);
	out[n++] = offsetof(struct ep_func, written);

	out[n++] = sizeof(struct ep_param);
	out[n++] = _Alignof(struct ep_param);
	out[n++] = offsetof(struct ep_param, type);
	out[n++] = offsetof(struct ep_param, name);
	out[n++] = offsetof(struct ep_param, is_const);
	out[n++] = offsetof(struct ep_param, is_uniform);
	out[n++] = offsetof(struct ep_param, written);
	out[n++] = offsetof(struct ep_param, array_count);

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

	out[n++] = sizeof(struct dstr);
	out[n++] = _Alignof(struct dstr);
	out[n++] = offsetof(struct dstr, array);
	out[n++] = offsetof(struct dstr, len);
	out[n++] = offsetof(struct dstr, capacity);
	return n;
}
