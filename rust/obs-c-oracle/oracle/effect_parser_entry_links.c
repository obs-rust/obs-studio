/* Test-only definitions, under their real names, of the libobs functions
 * the Rust ep_parse shim calls that no other test code defines, so the
 * test binaries link it, and so the C oracle (oracle/effect_parser_entry.c)
 * and the shim run the same code for each:
 *
 * - ep_check_for_keyword() and ep_parse_function(): the original
 *   libobs/graphics/effect-parser-funcs.c.h, compiled here with its
 *   lookups under oracle_entry_* names and on the C cf_adderror, strref_cmp
 *   and dstr of oracle/decl.c, oracle/lexer.c and oracle/dstr.c;
 * - cf_preprocess() and cf_preprocessor_add_def(): the C preprocessor of
 *   oracle/decl.c, under its oracle_* names;
 * - gs_preprocessor_name(): the backend name the test sets for its thread,
 *   counting the calls. */
#define ep_getstruct oracle_entry_ep_getstruct
#define ep_getsampler oracle_entry_ep_getsampler
#define ep_getfunc_strref oracle_entry_ep_getfunc_strref
#define ep_getparam_strref oracle_entry_ep_getparam_strref

#define cf_adderror oracle_cf_adderror
#define strref_cmp oracle_strref_cmp
#define dstr_copy oracle_dstr_copy
#define dstr_ncat oracle_dstr_ncat
#define dstr_cat_dstr oracle_dstr_cat_dstr
#define dstr_cat_strref oracle_dstr_cat_strref
#define dstr_catf oracle_dstr_catf

#include "graphics/effect-parser-funcs.c.h"

bool oracle_cf_preprocess(struct cf_preprocessor *pp, struct cf_lexer *lex, struct error_data *ed);
void oracle_cf_preprocessor_add_def(struct cf_preprocessor *pp, struct cf_def *def);

bool cf_preprocess(struct cf_preprocessor *pp, struct cf_lexer *lex, struct error_data *ed)
{
	return oracle_cf_preprocess(pp, lex, ed);
}

void cf_preprocessor_add_def(struct cf_preprocessor *pp, struct cf_def *def)
{
	oracle_cf_preprocessor_add_def(pp, def);
}

#if defined(_MSC_VER)
static __declspec(thread) const char *entry_backend;
static __declspec(thread) unsigned entry_name_calls;
#else
static _Thread_local const char *entry_backend;
static _Thread_local unsigned entry_name_calls;
#endif

const char *gs_preprocessor_name(void);
void oracle_effect_parser_entry_set_backend(const char *name);
unsigned oracle_effect_parser_entry_name_calls(void);

/* The name gs_preprocessor_name() hands back on this thread (NULL for
 * none), with its call count reset. */
void oracle_effect_parser_entry_set_backend(const char *name)
{
	entry_backend = name;
	entry_name_calls = 0;
}

/* How many times this thread called gs_preprocessor_name() since then. */
unsigned oracle_effect_parser_entry_name_calls(void)
{
	return entry_name_calls;
}

const char *gs_preprocessor_name(void)
{
	entry_name_calls++;
	return entry_backend;
}
