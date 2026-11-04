#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/effect-parser.h>
#include <util/bmem.h>

/*
 * Characterization of the effect parser's function parser and its lookups
 * (#3623): ep_parse() on an effect's text, then what it kept, written out one
 * line each, and compared whole:
 *
 *   struct <name>
 *   sampler <name>
 *   param <type> <name>
 *   func <return type> <name>[ : <mapping>]
 *     var <in|inout|out|uniform> <type> <name>[ : <mapping>]
 *     body <contents>
 *     deps struct=<names> func=<names> sampler=<names> param=<names>
 *   error <row>:<column> <level> <text>
 *   leaked <allocations>
 *
 * The structs, samplers and params by name only (the blocks and params tests
 * pin the rest), then each function in the order the parser pushed it: its
 * return type, name and mapping, its param vars (EP_VAR_IN is EP_VAR_NONE, so
 * a param with no keyword is "in"), its contents with each newline written \n
 * and each tab \t, and its four dependency lists in the order they were
 * pushed, a name once for each time it was. Then every error in the order it
 * was added (level 1 is a warning) and the allocations still held after
 * ep_free(). They pin the current C behaviour; cases marked "characterized,
 * not endorsed" are odd but real.
 *
 * ep_parse() and ep_free() are hidden in libobs, so this test compiles the
 * original effect parser C into itself (test-graph.json lists the files) and
 * links libobs for the rest. Every text is parsed behind a first line of
 * "{}", which ep_parse() reports as "Unexpected code segment": with an error
 * in the list it never calls ep_compile(), which needs a device. The rows
 * below count that line. No display and no graphics context, so it runs on
 * every platform: gs_preprocessor_name() is the no-context answer, NULL, so
 * no preprocessor name is defined.
 */

const char *gs_preprocessor_name(void);
const char *gs_preprocessor_name(void)
{
	return NULL;
}

struct func_case {
	const char *text;
	const char *expected;
};

static char dump_buf[16384];
static size_t dump_len;

static void dump_cat(const char *fmt, ...)
{
	va_list args;
	int n;

	va_start(args, fmt);
	n = vsnprintf(dump_buf + dump_len, sizeof(dump_buf) - dump_len, fmt, args);
	va_end(args);
	if (n > 0)
		dump_len += (size_t)n;
	if (dump_len >= sizeof(dump_buf))
		dump_len = sizeof(dump_buf) - 1;
}

static const char *str_or_null(const char *s)
{
	return s ? s : "(null)";
}

static const char *var_type_name(enum ep_var_type var_type)
{
	switch (var_type) {
	case EP_VAR_IN:
		return "in";
	case EP_VAR_INOUT:
		return "inout";
	case EP_VAR_OUT:
		return "out";
	case EP_VAR_UNIFORM:
		return "uniform";
	}
	return "(bad var type)";
}

static void dump_names(const char *label, char *const *names, size_t num)
{
	dump_cat(" %s=", label);
	for (size_t i = 0; i < num; i++)
		dump_cat(i ? ",%s" : "%s", str_or_null(names[i]));
}

static void dump_contents(const struct dstr *contents)
{
	for (size_t i = 0; i < contents->len; i++) {
		char c = contents->array[i];

		if (c == '\n')
			dump_cat("\\n");
		else if (c == '\t')
			dump_cat("\\t");
		else
			dump_cat("%c", c);
	}
}

static void dump_func(const struct ep_func *func)
{
	dump_cat("func %s %s", str_or_null(func->ret_type), str_or_null(func->name));
	if (func->mapping)
		dump_cat(" : %s", func->mapping);
	dump_cat("\n");

	for (size_t i = 0; i < func->param_vars.num; i++) {
		const struct ep_var *var = func->param_vars.array + i;

		dump_cat("  var %s %s %s", var_type_name(var->var_type), str_or_null(var->type),
			 str_or_null(var->name));
		if (var->mapping)
			dump_cat(" : %s", var->mapping);
		dump_cat("\n");
	}

	dump_cat("  body ");
	dump_contents(&func->contents);
	dump_cat("\n");

	dump_cat("  deps");
	dump_names("struct", func->struct_deps.array, func->struct_deps.num);
	dump_names("func", func->func_deps.array, func->func_deps.num);
	dump_names("sampler", func->sampler_deps.array, func->sampler_deps.num);
	dump_names("param", func->param_deps.array, func->param_deps.num);
	dump_cat("\n");
}

static void dump_parser(struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->structs.num; i++)
		dump_cat("struct %s\n", str_or_null(ep->structs.array[i].name));
	for (size_t i = 0; i < ep->samplers.num; i++)
		dump_cat("sampler %s\n", str_or_null(ep->samplers.array[i].name));
	for (size_t i = 0; i < ep->params.num; i++)
		dump_cat("param %s %s\n", str_or_null(ep->params.array[i].type), str_or_null(ep->params.array[i].name));
	for (size_t i = 0; i < ep->funcs.num; i++)
		dump_func(ep->funcs.array + i);
	for (size_t i = 0; i < ep->cfp.error_list.errors.num; i++) {
		struct error_item *err = ep->cfp.error_list.errors.array + i;

		dump_cat("error %u:%u %d %s\n", (unsigned)err->row, (unsigned)err->column, err->level,
			 str_or_null(err->error));
	}
}

static void check_cases(const struct func_case *cases, size_t count)
{
	size_t failed = 0;
	char text[2048];

	for (size_t i = 0; i < count; i++) {
		struct effect_parser ep;
		long before;
		bool ok;

		snprintf(text, sizeof(text), "{}\n%s", cases[i].text);

		dump_len = 0;
		dump_buf[0] = 0;

		before = bnum_allocs();
		ep_init(&ep);
		ok = ep_parse(&ep, NULL, text, "test_effect_parser_funcs");
		if (ok)
			dump_cat("ok 1 (the guard error is missing)\n");
		dump_parser(&ep);
		ep_free(&ep);
		dump_cat("leaked %ld\n", bnum_allocs() - before);

		if (strcmp(dump_buf, cases[i].expected) != 0) {
			fprintf(stderr, "case %zu: %s\n--- expected\n%s--- got\n%s", i, cases[i].text,
				cases[i].expected, dump_buf);
			failed++;
		}
	}

	if (failed) {
		fprintf(stderr, "%zu of %zu cases differ\n", failed, count);
		fail();
	}
}

#define CHECK_CASES(cases) check_cases(cases, sizeof(cases) / sizeof(cases[0]))

/* The guard line's error, first in every case. */
#define GUARD "error 1:1 0 Unexpected code segment\n"

/* ------------------------------------------------------------------------ */

static const struct func_case signatures_cases[] = {
	{"float4 f() { return 1; }", "func float4 f\n"
				     "  body { return 1; }\n"
				     "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	/* Whitespace before the ')' is skipped by the peek. */
	{"float4 f( ) { }", "func float4 f\n"
			    "  body { }\n"
			    "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	{"void f(float a) {}", "func void f\n"
			       "  var in float a\n"
			       "  body {}\n"
			       "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	{"float4 f(in float a, inout float b, out float c, uniform float d) {}",
	 "func float4 f\n"
	 "  var in float a\n"
	 "  var inout float b\n"
	 "  var out float c\n"
	 "  var uniform float d\n"
	 "  body {}\n"
	 "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	{"float4 f(float4 pos : POSITION, float2 uv : TEXCOORD0) : TARGET { return pos; }",
	 "func float4 f : TARGET\n"
	 "  var in float4 pos : POSITION\n"
	 "  var in float2 uv : TEXCOORD0\n"
	 "  body { return pos; }\n"
	 "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	/* Each param takes at most one of the four keywords: the second is read
	 * as its type, the type as its name, and the name is a syntax error. The
	 * next param then fails on the ')', whose skip to a ')' starts after it,
	 * so the function and the rest of the text are dropped (characterized,
	 * not endorsed). */
	{"float4 f(in out float a) {}", GUARD "error 2:23 0 Syntax error\n"
					      "error 2:24 0 Expected 'type'\n"
					      "error 2:28 0 Unexpected EOF\n"
					      "leaked 0\n"},
	{"float4 f(in in float a) {}", GUARD "error 2:22 0 Syntax error\n"
					     "error 2:23 0 Expected 'type'\n"
					     "error 2:27 0 Unexpected EOF\n"
					     "leaked 0\n"},
	{"float4 f(uniform inout float a) {}", GUARD "error 2:30 0 Syntax error\n"
						     "error 2:31 0 Expected 'type'\n"
						     "error 2:35 0 Unexpected EOF\n"
						     "leaked 0\n"},
	/* A ',' before the ')' starts a param that fails on the ')', with the
	 * same skip past it (characterized, not endorsed). */
	{"float4 f(float a,) {}", GUARD "error 2:18 0 Expected 'type'\n"
					"error 2:22 0 Unexpected EOF\n"
					"leaked 0\n"},
	/* An error before the ')' skips to it, and the function is kept without
	 * that param. */
	{"float4 f(,float a) {}", "func float4 f\n"
				  "  body {}\n"
				  "  deps struct= func= sampler= param=\n" GUARD "error 2:10 0 Expected 'type'\n"
				  "leaked 0\n"},
	/* A token that is neither ',' nor ')' after a param is reported, and the
	 * next param starts after it (here failing on the ')' as above). */
	{"float4 f(float a b) {}", GUARD "error 2:18 0 Syntax error\n"
					 "error 2:19 0 Expected 'type'\n"
					 "error 2:23 0 Unexpected EOF\n"
					 "leaked 0\n"},
	{"float4 f(float a b c) {}", GUARD "error 2:18 0 Syntax error\n"
					   "error 2:21 0 Expected 'name'\n"
					   "error 2:25 0 Unexpected EOF\n"
					   "leaked 0\n"},
	{"float4 f(uniform) {}", GUARD "error 2:17 0 Expected 'type'\n"
				       "error 2:21 0 Unexpected EOF\n"
				       "leaked 0\n"},
	{"float4 f(float) {}", GUARD "error 2:15 0 Expected 'name'\n"
				     "error 2:19 0 Unexpected EOF\n"
				     "leaked 0\n"},
	{"float4 f(float 1) {}", "func float4 f\n"
				 "  body {}\n"
				 "  deps struct= func= sampler= param=\n" GUARD "error 2:16 0 Expected 'name'\n"
				 "leaked 0\n"},
	{"float4 f(1 a) {}", "func float4 f\n"
			     "  body {}\n"
			     "  deps struct= func= sampler= param=\n" GUARD "error 2:10 0 Expected 'type'\n"
			     "leaked 0\n"},
	{"float4 f(float a, 1 b, float c) {}",
	 "func float4 f\n"
	 "  var in float a\n"
	 "  body {}\n"
	 "  deps struct= func= sampler= param=\n" GUARD "error 2:19 0 Expected 'type'\n"
	 "leaked 0\n"},
	{"float4 f(float a :) {}", GUARD "error 2:19 0 Expected 'mapping specifier'\n"
					 "error 2:23 0 Unexpected EOF\n"
					 "leaked 0\n"},
	{"float4 f(float a : 1) {}",
	 "func float4 f\n"
	 "  body {}\n"
	 "  deps struct= func= sampler= param=\n" GUARD "error 2:20 0 Expected 'mapping specifier'\n"
	 "leaked 0\n"},
	{"float4 f(float a : M N) {}", GUARD "error 2:22 0 Syntax error\n"
					     "error 2:23 0 Expected 'type'\n"
					     "error 2:27 0 Unexpected EOF\n"
					     "leaked 0\n"},
	{"float4 f(float a) : COLOR {}", "func float4 f : COLOR\n"
					 "  var in float a\n"
					 "  body {}\n"
					 "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	/* A missing mapping name on the '{' skips past it, to the end of the
	 * text (characterized, not endorsed). */
	{"float4 f(float a) : {}", GUARD "error 2:21 0 Expected 'mapping specifier'\n"
					 "error 2:23 0 Unexpected EOF\n"
					 "leaked 0\n"},
	{"float4 f(float a) : 1 {}",
	 "func float4 f\n"
	 "  var in float a\n"
	 "  body {}\n"
	 "  deps struct= func= sampler= param=\n" GUARD "error 2:21 0 Expected 'mapping specifier'\n"
	 "leaked 0\n"},
	{"float4 f(float a) : COLOR DEPTH {}", GUARD "error 2:27 0 Expected '{'\n"
						     "error 2:33 0 Expected 'name'\n"
						     "error 2:35 0 Unexpected EOF\n"
						     "leaked 0\n"},
	/* Without its '{' the function is dropped, and the parser goes on from
	 * that token, here as a param declaration (characterized, not endorsed). */
	{"float4 f(float a) return a; }\nuniform float g;", "param return a\n" GUARD "error 2:19 0 Expected '{'\n"
							    "error 2:29 0 Expected 'type'\n"
							    "leaked 0\n"},
	{"float4 f(float a); uniform float g;", "param float g\n" GUARD "error 2:18 0 Expected '{'\n"
						"leaked 0\n"},
	/* Two functions of the same name are both kept. */
	{"float4 f() {}\nfloat4 f() {}", "func float4 f\n"
					 "  body {}\n"
					 "  deps struct= func= sampler= param=\n"
					 "func float4 f\n"
					 "  body {}\n"
					 "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
};

static void signatures(void **state)
{
	(void)state;
	CHECK_CASES(signatures_cases);
}

/* ------------------------------------------------------------------------ */

static const struct func_case bodies_cases[] = {
	{"float4 f() { if (a) { b; } { } return c; }", "func float4 f\n"
						       "  body { if (a) { b; } { } return c; }\n"
						       "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	{"float4 f()\n{\n\treturn 1;\n}\n", "func float4 f\n"
					    "  body {\\n\\treturn 1;\\n}\n"
					    "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	{"float4 f() { /* c */ return 1; // d\n}", "func float4 f\n"
						   "  body {   return 1;  \\n}\n"
						   "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	{"float4 f() { return 1; } uniform float g;", "param float g\n"
						      "func float4 f\n"
						      "  body { return 1; }\n"
						      "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	{"float4 f() { return \"}\"; }", "func float4 f\n"
					 "  body { return \"}\"; }\n"
					 "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	/* A '}' right after the '{' ends the body. */
	{"float4 f() {}}", "func float4 f\n"
			   "  body {}\n"
			   "  deps struct= func= sampler= param=\n" GUARD "error 2:14 0 Expected 'type'\n"
			   "error 2:15 0 Unexpected EOF\n"
			   "leaked 0\n"},
};

static void bodies(void **state)
{
	(void)state;
	CHECK_CASES(bodies_cases);
}

/* ------------------------------------------------------------------------ */

static const struct func_case deps_cases[] = {
	/* A struct return type, param type and body name are struct deps, each
	 * time it appears. */
	{"struct S { float4 a : POSITION; };\nS f(S s) { S t; t.a = s.a; return t; }",
	 "struct S\n"
	 "func S f\n"
	 "  var in S s\n"
	 "  body { S t; t.a = s.a; return t; }\n"
	 "  deps struct=S,S,S func= sampler= param=\n" GUARD "leaked 0\n"},
	{"sampler_state def { Filter = Linear; };\nfloat4 f(texture2d t, float2 uv) { return t.Sample(def, uv); }",
	 "sampler def\n"
	 "func float4 f\n"
	 "  var in texture2d t\n"
	 "  var in float2 uv\n"
	 "  body { return t.Sample(def, uv); }\n"
	 "  deps struct= func= sampler=def param=\n" GUARD "leaked 0\n"},
	{"sampler_state def { Filter = Linear; };\nfloat4 f(def s) { return s; }",
	 "sampler def\n"
	 "func float4 f\n"
	 "  var in def s\n"
	 "  body { return s; }\n"
	 "  deps struct= func= sampler=def param=\n" GUARD "leaked 0\n"},
	{"uniform float4 color;\nfloat4 f() { return color * color; }",
	 "param float4 color\n"
	 "func float4 f\n"
	 "  body { return color * color; }\n"
	 "  deps struct= func= sampler= param=color,color\n" GUARD "leaked 0\n"},
	/* A field named like a param is a param dep too. */
	{"uniform float4 color;\nstruct S { float4 color; };\nfloat4 f(S s) { return s.color; }",
	 "struct S\n"
	 "param float4 color\n"
	 "func float4 f\n"
	 "  var in S s\n"
	 "  body { return s.color; }\n"
	 "  deps struct=S func= sampler= param=color\n" GUARD "leaked 0\n"},
	{"float4 g() { return 1; }\nfloat4 f() { return g() + g(); }",
	 "func float4 g\n"
	 "  body { return 1; }\n"
	 "  deps struct= func= sampler= param=\n"
	 "func float4 f\n"
	 "  body { return g() + g(); }\n"
	 "  deps struct= func=g,g sampler= param=\n" GUARD "leaked 0\n"},
	/* A function is not its own dep: it is pushed only after its body. */
	{"float4 f() { return f(); }", "func float4 f\n"
				       "  body { return f(); }\n"
				       "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	{"float4 f() { return later; }\nuniform float later;",
	 "param float later\n"
	 "func float4 f\n"
	 "  body { return later; }\n"
	 "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
	/* A struct is looked up before a param of the same name. */
	{"struct A { float4 a; };\nuniform float A;\nfloat4 f() { return A; }",
	 "struct A\n"
	 "param float A\n"
	 "func float4 f\n"
	 "  body { return A; }\n"
	 "  deps struct=A func= sampler= param=\n" GUARD "leaked 0\n"},
	/* A function is looked up before a sampler of the same name. */
	{"float4 def() { return 1; }\nsampler_state def { Filter = Linear; };\nfloat4 f() { return def; }",
	 "sampler def\n"
	 "func float4 def\n"
	 "  body { return 1; }\n"
	 "  deps struct= func= sampler= param=\n"
	 "func float4 f\n"
	 "  body { return def; }\n"
	 "  deps struct= func=def sampler= param=\n" GUARD "leaked 0\n"},
	/* A sampler is looked up before a param of the same name. */
	{"sampler_state s { Filter = Linear; };\nuniform float s;\nfloat4 f() { return s; }",
	 "sampler s\n"
	 "param float s\n"
	 "func float4 f\n"
	 "  body { return s; }\n"
	 "  deps struct= func= sampler=s param=\n" GUARD "leaked 0\n"},
	/* A param type that is a struct and a sampler is a struct dep only. */
	{"struct T { float a; };\nsampler_state T { Filter = Linear; };\nfloat4 f(T t) { return 1; }",
	 "struct T\n"
	 "sampler T\n"
	 "func float4 f\n"
	 "  var in T t\n"
	 "  body { return 1; }\n"
	 "  deps struct=T func= sampler= param=\n" GUARD "leaked 0\n"},
	{"struct S { float4 a; };\nsampler_state def { Filter = Linear; };\nuniform float4 c;\nfloat4 g(S s) { return s.a; }\nS h(S s, def d) : TARGET { S r; r.a = g(s) * c + t.Sample(def, s.a); return r; }",
	 "struct S\n"
	 "sampler def\n"
	 "param float4 c\n"
	 "func float4 g\n"
	 "  var in S s\n"
	 "  body { return s.a; }\n"
	 "  deps struct=S func= sampler= param=\n"
	 "func S h : TARGET\n"
	 "  var in S s\n"
	 "  var in def d\n"
	 "  body { S r; r.a = g(s) * c + t.Sample(def, s.a); return r; }\n"
	 "  deps struct=S,S,S func=g sampler=def,def param=c\n" GUARD "leaked 0\n"},
};

static void deps(void **state)
{
	(void)state;
	CHECK_CASES(deps_cases);
}

/* ------------------------------------------------------------------------ */

static const struct func_case eof_cases[] = {
	{"float4 f(", GUARD "error 2:9 0 Unexpected EOF\n"
			    "error 2:9 0 Expected 'type'\n"
			    "error 2:10 0 Unexpected EOF\n"
			    "leaked 0\n"},
	{"float4 f( ", GUARD "error 2:9 0 Unexpected EOF\n"
			     "error 2:9 0 Expected 'type'\n"
			     "error 2:11 0 Unexpected EOF\n"
			     "leaked 0\n"},
	{"float4 f(in", GUARD "error 2:12 0 Unexpected EOF\n"
			      "leaked 0\n"},
	{"float4 f(in float", GUARD "error 2:18 0 Unexpected EOF\n"
				    "leaked 0\n"},
	{"float4 f(in float a", GUARD "error 2:20 0 Unexpected EOF\n"
				      "leaked 0\n"},
	{"float4 f(in float a :", GUARD "error 2:22 0 Unexpected EOF\n"
					"leaked 0\n"},
	{"float4 f(in float a : M", GUARD "error 2:24 0 Unexpected EOF\n"
					  "leaked 0\n"},
	{"float4 f(in float a : M,", GUARD "error 2:25 0 Unexpected EOF\n"
					   "leaked 0\n"},
	{"float4 f(float a, float b", GUARD "error 2:26 0 Unexpected EOF\n"
					    "leaked 0\n"},
	{"float4 f(1", GUARD "error 2:10 0 Expected 'type'\n"
			     "error 2:11 0 Unexpected EOF\n"
			     "leaked 0\n"},
	{"float4 f(float 1", GUARD "error 2:16 0 Expected 'name'\n"
				   "error 2:17 0 Unexpected EOF\n"
				   "leaked 0\n"},
	{"float4 f()", GUARD "error 2:11 0 Unexpected EOF\n"
			     "leaked 0\n"},
	{"float4 f(in float a)", GUARD "error 2:21 0 Unexpected EOF\n"
				       "leaked 0\n"},
	{"float4 f(in float a) :", GUARD "error 2:23 0 Unexpected EOF\n"
					 "leaked 0\n"},
	{"float4 f(in float a) : 1", GUARD "error 2:24 0 Expected 'mapping specifier'\n"
					   "error 2:25 0 Unexpected EOF\n"
					   "leaked 0\n"},
	{"float4 f(in float a) : C", GUARD "error 2:25 0 Unexpected EOF\n"
					   "leaked 0\n"},
	{"float4 f(in float a) : C { x }", "func float4 f : C\n"
					   "  var in float a\n"
					   "  body { x }\n"
					   "  deps struct= func= sampler= param=\n" GUARD "leaked 0\n"},
};

static void eof(void **state)
{
	(void)state;
	CHECK_CASES(eof_cases);
}

/* ------------------------------------------------------------------------ */

static const struct func_case keywords_cases[] = {
	{"uniform uniform float x;", "param float x\n" GUARD "error 2:17 1 'uniform' keyword already specified\n"
				     "leaked 0\n"},
	{"const const property property float x;",
	 "param float x\n" GUARD "error 2:13 1 'const' keyword already specified\n"
	 "error 2:31 1 'property' keyword already specified\n"
	 "leaked 0\n"},
	{"uniform const uniform float x;", "param float x\n" GUARD "error 2:23 1 'uniform' keyword already specified\n"
					   "leaked 0\n"},
	{"uniform float4 f() {}", "func float4 f\n"
				  "  body {}\n"
				  "  deps struct= func= sampler= param=\n" GUARD
				  "error 2:17 0 'uniform' keyword cannot be used with a function\n"
				  "leaked 0\n"},
	{"const property uniform float4 f(in float a) {}",
	 "func float4 f\n"
	 "  var in float a\n"
	 "  body {}\n"
	 "  deps struct= func= sampler= param=\n" GUARD
	 "error 2:32 0 'property' keyword cannot be used with a function\n"
	 "error 2:32 0 'const' keyword cannot be used with a function\n"
	 "error 2:32 0 'uniform' keyword cannot be used with a function\n"
	 "leaked 0\n"},
	{"property const property float4 f() {}",
	 "func float4 f\n"
	 "  body {}\n"
	 "  deps struct= func= sampler= param=\n" GUARD "error 2:25 1 'property' keyword already specified\n"
	 "error 2:33 0 'property' keyword cannot be used with a function\n"
	 "error 2:33 0 'const' keyword cannot be used with a function\n"
	 "leaked 0\n"},
	{"uniform", GUARD "error 2:8 0 Unexpected EOF\n"
			  "leaked 0\n"},
	{"uniform const", GUARD "error 2:14 0 Unexpected EOF\n"
				"leaked 0\n"},
	{"uniform uniform", GUARD "error 2:16 0 Unexpected EOF\n"
				  "leaked 0\n"},
};

static void keywords(void **state)
{
	(void)state;
	CHECK_CASES(keywords_cases);
}

/* ------------------------------------------------------------------------ */

static const struct func_case mixed_cases[] = {
	{"uniform float4x4 ViewProj;\nuniform texture2d image;\nsampler_state def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Clamp; };\nstruct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\nVertInOut VSDefault(VertInOut vert_in)\n{\n\tVertInOut vert_out;\n\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n\tvert_out.uv = vert_in.uv;\n\treturn vert_out;\n}\nfloat4 PSDrawBare(VertInOut vert_in) : TARGET\n{\n\treturn image.Sample(def_sampler, vert_in.uv);\n}\ntechnique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSDrawBare(vert_in); } }",
	 "struct VertInOut\n"
	 "sampler def_sampler\n"
	 "param float4x4 ViewProj\n"
	 "param texture2d image\n"
	 "func VertInOut VSDefault\n"
	 "  var in VertInOut vert_in\n"
	 "  body {\\n\\tVertInOut vert_out;\\n\\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\\n\\tvert_out.uv = vert_in.uv;\\n\\treturn vert_out;\\n}\n"
	 "  deps struct=VertInOut,VertInOut,VertInOut func= sampler= param=ViewProj\n"
	 "func float4 PSDrawBare : TARGET\n"
	 "  var in VertInOut vert_in\n"
	 "  body {\\n\\treturn image.Sample(def_sampler, vert_in.uv);\\n}\n"
	 "  deps struct=VertInOut func= sampler=def_sampler param=image\n" GUARD "leaked 0\n"},
};

static void mixed(void **state)
{
	(void)state;
	CHECK_CASES(mixed_cases);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(signatures), cmocka_unit_test(bodies),   cmocka_unit_test(deps),
		cmocka_unit_test(eof),        cmocka_unit_test(keywords), cmocka_unit_test(mixed),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
