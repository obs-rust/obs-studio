/* Test-only oracle: the original libobs/graphics/effect-parser-entry.c.h,
 * unmodified, with ep_parse renamed to oracle_ep_parse so it can link next
 * to the Rust shim (its helpers are static). It runs on the original C
 * block, technique and parameter parsers of oracle/effect_parser_blocks.c,
 * oracle/effect_parser_passes.c and oracle/effect_parser_params.c, and on
 * the C cf-parser inlines with the C cf_adderror, cf_pass_pair, lexer and
 * preprocessor of oracle/decl.c and the C strref_cmp of oracle/lexer.c,
 * under their oracle_* names, so the whole walk is the original C. The
 * calls the Rust shim makes under their real names go to the same code:
 * the backend name stand-in, ep_check_for_keyword() and the C function
 * parser (oracle/effect_parser_entry_links.c), and the compile step (the
 * production ep_compile, over the recording graphics stand-in of
 * oracle/effect_parser_compile_device.c). bmalloc stays the real symbol:
 * the Rust port and this oracle share it.
 *
 * The helpers below set up a parser and its effect (ep_init() and
 * effect_init(), with the C lexer and preprocessor), free them (ep_free()
 * and effect_free()), and write out everything the parse leaves in them,
 * so the parity test can compare both sides. */
#define ep_parse oracle_ep_parse

#define ep_parse_struct oracle_ep_parse_struct
#define ep_parse_sampler_state oracle_ep_parse_sampler_state
#define ep_parse_technique oracle_ep_parse_technique
#define ep_parse_param oracle_ep_parse_param

#define cf_adderror oracle_cf_adderror
#define cf_pass_pair oracle_cf_pass_pair
#define cf_preprocess oracle_cf_preprocess
#define cf_preprocessor_init oracle_cf_preprocessor_init
#define cf_preprocessor_free oracle_cf_preprocessor_free
#define cf_preprocessor_add_def oracle_cf_preprocessor_add_def
#define cf_lexer_init oracle_cf_lexer_init
#define cf_lexer_free oracle_cf_lexer_free
#define cf_lexer_lex oracle_cf_lexer_lex
#define strref_cmp oracle_strref_cmp

#include "graphics/effect-parser-entry.c.h"

#include "graphics/effect.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

void oracle_effect_parser_entry_init(struct effect_parser *ep, gs_effect_t *effect);
void oracle_effect_parser_entry_free(struct effect_parser *ep, gs_effect_t *effect);
char *oracle_effect_parser_entry_dump(struct effect_parser *ep, gs_effect_t *effect);
void oracle_effect_parser_entry_dump_free(char *dump);
size_t oracle_effect_parser_entry_layout(size_t *out);

/* ep_init() and effect_init(): a parser and an effect as
 * gs_effect_create() hands them to ep_parse(). */
void oracle_effect_parser_entry_init(struct effect_parser *ep, gs_effect_t *effect)
{
	ep_init(ep);
	effect_init(effect);
}

/* ep_free() (the production one, as gs_effect_create() calls it) and
 * effect_free(). */
void oracle_effect_parser_entry_free(struct effect_parser *ep, gs_effect_t *effect)
{
	ep_free(ep);
	effect_free(effect);
}

struct dump {
	char *array;
	size_t len, capacity;
};

#if defined(__GNUC__) || defined(__clang__)
#define DUMP_FORMAT __attribute__((format(printf, 2, 3)))
#else
#define DUMP_FORMAT
#endif

static void out(struct dump *d, const char *fmt, ...) DUMP_FORMAT;
static void out(struct dump *d, const char *fmt, ...)
{
	va_list args;
	int n;

	va_start(args, fmt);
	n = vsnprintf(NULL, 0, fmt, args);
	va_end(args);
	if (n <= 0)
		return;
	if (d->len + (size_t)n + 1 > d->capacity) {
		size_t capacity = (d->capacity ? d->capacity * 2 : 4096) + (size_t)n;
		char *grown = realloc(d->array, capacity);
		if (!grown)
			abort();
		d->array = grown;
		d->capacity = capacity;
	}
	va_start(args, fmt);
	vsnprintf(d->array + d->len, d->capacity - d->len, fmt, args);
	va_end(args);
	d->len += (size_t)n;
}

static const char *s(const char *str)
{
	return str ? str : "(null)";
}

static void out_strref(struct dump *d, const struct strref *ref)
{
	if (!ref->array) {
		out(d, "(null)");
		return;
	}
	out(d, "\"%.*s\"", (int)ref->len, ref->array);
}

static void out_param(struct dump *d, gs_effect_t *effect, const struct ep_param *p, bool annotation)
{
	const char *indent = annotation ? "    " : "";

	out(d, "%sparam %s %s const=%d property=%d uniform=%d texture=%d written=%d order=%d count=%d default=", indent,
	    s(p->type), s(p->name), p->is_const, p->is_property, p->is_uniform, p->is_texture, p->written,
	    p->writeorder, p->array_count);
	for (size_t i = 0; i < p->default_val.num; i++)
		out(d, "%02x", p->default_val.array[i]);
	out(d, " properties=%zu", p->properties.num);
	/* ep_param_init() leaves param unset (the Rust parameter shim writes
	 * NULL) until the compile step links it, which it never does for an
	 * annotation: only a compiled param's link is compared. */
	if (annotation || !effect->params.num)
		out(d, " link=?");
	else if (!p->param)
		out(d, " link=-");
	else if (effect->params.num && p->param >= effect->params.array &&
		 p->param < effect->params.array + effect->params.num)
		out(d, " link=%zu", (size_t)(p->param - effect->params.array));
	else
		out(d, " link=other");
	out(d, "\n");
	for (size_t i = 0; i < p->annotations.num; i++)
		out_param(d, effect, p->annotations.array + i, true);
}

static void out_vars(struct dump *d, const ep_var_array_t *vars)
{
	for (size_t i = 0; i < vars->num; i++) {
		const struct ep_var *v = vars->array + i;
		out(d, "    var %s %s %s %d\n", s(v->type), s(v->name), s(v->mapping), (int)v->var_type);
	}
}

static void out_names(struct dump *d, const char *label, char *const *names, size_t num)
{
	out(d, "    %s", label);
	for (size_t i = 0; i < num; i++)
		out(d, " %s", s(names[i]));
	out(d, "\n");
}

static void out_program(struct dump *d, const char *label, const cf_token_array_t *program)
{
	out(d, "    %s", label);
	for (size_t i = 0; i < program->num; i++) {
		out(d, " %d:", program->array[i].type);
		out_strref(d, &program->array[i].str);
	}
	out(d, "\n");
}

/* Everything ep_parse() leaves in the parser and the effect, as text: the
 * effect pointer, where cur_token stopped, every param (with its
 * annotations and the effect param it links to), struct, sampler, function
 * and technique, every error, the preprocessor's definitions, and the
 * effect's params, techniques and passes. Free it with
 * oracle_effect_parser_entry_dump_free(). */
char *oracle_effect_parser_entry_dump(struct effect_parser *ep, gs_effect_t *effect)
{
	struct dump d = {0};
	struct cf_token *tokens = cf_preprocessor_get_tokens(&ep->cfp.pp);

	out(&d, "effect %s\n", ep->effect == effect ? "set" : (ep->effect ? "other" : "null"));
	if (!ep->cfp.cur_token)
		out(&d, "cur null\n");
	else if (tokens)
		out(&d, "cur %td type %d\n", ep->cfp.cur_token - tokens, ep->cfp.cur_token->type);
	else
		out(&d, "cur other\n");

	for (size_t i = 0; i < ep->params.num; i++)
		out_param(&d, effect, ep->params.array + i, false);

	for (size_t i = 0; i < ep->structs.num; i++) {
		const struct ep_struct *st = ep->structs.array + i;
		out(&d, "struct %s written=%d\n", s(st->name), st->written);
		out_vars(&d, &st->vars);
	}

	for (size_t i = 0; i < ep->samplers.num; i++) {
		const struct ep_sampler *sa = ep->samplers.array + i;
		out(&d, "sampler %s written=%d\n", s(sa->name), sa->written);
		out_names(&d, "states", sa->states.array, sa->states.num);
		out_names(&d, "values", sa->values.array, sa->values.num);
	}

	for (size_t i = 0; i < ep->funcs.num; i++) {
		const struct ep_func *f = ep->funcs.array + i;
		out(&d, "func %s %s %s written=%d\n", s(f->name), s(f->ret_type), s(f->mapping), f->written);
		out(&d, "    contents %s\n", s(f->contents.array));
		out_vars(&d, &f->param_vars);
		out_names(&d, "func_deps", f->func_deps.array, f->func_deps.num);
		out_names(&d, "struct_deps", f->struct_deps.array, f->struct_deps.num);
		out_names(&d, "param_deps", f->param_deps.array, f->param_deps.num);
		out_names(&d, "sampler_deps", f->sampler_deps.array, f->sampler_deps.num);
	}

	for (size_t i = 0; i < ep->techniques.num; i++) {
		const struct ep_technique *t = ep->techniques.array + i;
		out(&d, "technique %s\n", s(t->name));
		for (size_t j = 0; j < t->passes.num; j++) {
			const struct ep_pass *p = t->passes.array + j;
			out(&d, "  pass %s linked=%d\n", s(p->name), p->pass != NULL);
			out_program(&d, "vertex", &p->vertex_program);
			out_program(&d, "fragment", &p->fragment_program);
		}
	}

	for (size_t i = 0; i < ep->cfp.error_list.errors.num; i++) {
		const struct error_item *e = ep->cfp.error_list.errors.array + i;
		out(&d, "error %s %s %u:%u %d\n", s(e->error), s(e->file), e->row, e->column, e->level);
	}

	for (size_t i = 0; i < ep->cfp.pp.defines.num; i++) {
		const struct cf_def *def = ep->cfp.pp.defines.array + i;
		out(&d, "define ");
		out_strref(&d, &def->name.str);
		out(&d, " ");
		out_strref(&d, &def->name.unmerged_str);
		out(&d, " type=%d params=%zu tokens=%zu macro=%d\n", def->name.type, def->params.num, def->tokens.num,
		    def->macro);
	}

	out(&d, "effect params %zu techniques %zu\n", effect->params.num, effect->techniques.num);
	for (size_t i = 0; i < effect->params.num; i++) {
		const struct gs_effect_param *p = effect->params.array + i;
		out(&d, "  eparam %s type=%d section=%d default=%zu annotations=%zu\n", s(p->name), (int)p->type,
		    (int)p->section, p->default_val.num, p->annotations.num);
	}
	for (size_t i = 0; i < effect->techniques.num; i++) {
		const struct gs_effect_technique *t = effect->techniques.array + i;
		out(&d, "  etechnique %s passes=%zu\n", s(t->name), t->passes.num);
		for (size_t j = 0; j < t->passes.num; j++) {
			const struct gs_effect_pass *p = t->passes.array + j;
			out(&d, "    epass %s vert=%p pixel=%p vparams=%zu pparams=%zu\n", s(p->name),
			    (void *)p->vertshader, (void *)p->pixelshader, p->vertshader_params.num,
			    p->pixelshader_params.num);
		}
	}

	if (!d.array)
		out(&d, "%s", "");
	return d.array;
}

void oracle_effect_parser_entry_dump_free(char *dump)
{
	free(dump);
}

/* size, align, then the offset of every field, of struct cf_def in
 * declaration order. Returns how many values it wrote. */
size_t oracle_effect_parser_entry_layout(size_t *out_values)
{
	size_t n = 0;

	out_values[n++] = sizeof(struct cf_def);
	out_values[n++] = _Alignof(struct cf_def);
	out_values[n++] = offsetof(struct cf_def, name);
	out_values[n++] = offsetof(struct cf_def, params);
	out_values[n++] = offsetof(struct cf_def, tokens);
	out_values[n++] = offsetof(struct cf_def, macro);
	return n;
}
