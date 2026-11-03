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
 * Characterization of the effect parser's struct and sampler-state blocks
 * (#3550): ep_parse() on an effect's text, then what it kept, written out
 * one line each, and compared whole:
 *
 *   struct <name> { <type> <name>[ : <mapping>]; ... }
 *   sampler <name> { <state> = <value>; ... }
 *   param <type> <name>
 *   func <return type> <name>
 *   error <row>:<column> <level> <text>
 *   leaked <allocations>
 *
 * Structs, samplers, params and functions in the order the parser pushed
 * them, then every error in the order it was added, then the allocations
 * still held after ep_free(). The params show where the parser went on
 * after a block. They pin the current C behaviour; cases marked
 * "characterized, not endorsed" are odd but real.
 *
 * ep_parse() and ep_free() are hidden in libobs, so this test compiles the
 * original effect parser C into itself (the cmake lists the files) and links
 * libobs for the rest. Every text is parsed behind a first line of "{}",
 * which ep_parse() reports as "Unexpected code segment": with an error in
 * the list it never calls ep_compile(), which needs a device. The rows below
 * count that line. No display and no graphics context, so it runs on every
 * platform: gs_preprocessor_name() is the no-context answer, NULL, so no
 * preprocessor name is defined.
 */

const char *gs_preprocessor_name(void);
const char *gs_preprocessor_name(void)
{
	return NULL;
}

struct block_case {
	const char *text;
	const char *expected;
};

static char dump_buf[4096];
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

static void dump_parser(struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->structs.num; i++) {
		struct ep_struct *st = ep->structs.array + i;

		dump_cat("struct %s {", str_or_null(st->name));
		for (size_t j = 0; j < st->vars.num; j++) {
			struct ep_var *var = st->vars.array + j;

			dump_cat(" %s %s", str_or_null(var->type), str_or_null(var->name));
			if (var->mapping)
				dump_cat(" : %s", var->mapping);
			if (var->var_type != EP_VAR_NONE)
				dump_cat(" (var type %d)", (int)var->var_type);
			dump_cat(";");
		}
		if (st->written)
			dump_cat(" (written)");
		dump_cat(" }\n");
	}
	for (size_t i = 0; i < ep->samplers.num; i++) {
		struct ep_sampler *es = ep->samplers.array + i;

		dump_cat("sampler %s {", str_or_null(es->name));
		for (size_t j = 0; j < es->states.num; j++)
			dump_cat(" %s = %s;", str_or_null(es->states.array[j]),
				 j < es->values.num ? str_or_null(es->values.array[j]) : "(none)");
		if (es->values.num != es->states.num)
			dump_cat(" (%zu values)", es->values.num);
		if (es->written)
			dump_cat(" (written)");
		dump_cat(" }\n");
	}
	for (size_t i = 0; i < ep->params.num; i++)
		dump_cat("param %s %s\n", str_or_null(ep->params.array[i].type), str_or_null(ep->params.array[i].name));
	for (size_t i = 0; i < ep->funcs.num; i++)
		dump_cat("func %s %s\n", str_or_null(ep->funcs.array[i].ret_type),
			 str_or_null(ep->funcs.array[i].name));
	for (size_t i = 0; i < ep->cfp.error_list.errors.num; i++) {
		struct error_item *err = ep->cfp.error_list.errors.array + i;

		dump_cat("error %u:%u %d %s\n", (unsigned)err->row, (unsigned)err->column, err->level,
			 str_or_null(err->error));
	}
}

static void check_cases(const struct block_case *cases, size_t count)
{
	size_t failed = 0;
	char text[1024];

	for (size_t i = 0; i < count; i++) {
		struct effect_parser ep;
		long before;
		bool ok;

		snprintf(text, sizeof(text), "{}\n%s", cases[i].text);

		dump_len = 0;
		dump_buf[0] = 0;

		before = bnum_allocs();
		ep_init(&ep);
		ok = ep_parse(&ep, NULL, text, "test_effect_parser_blocks");
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

static const struct block_case struct_cases[] = {
	{"struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\nuniform float4 c;",
	 "struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; }\n"
	 "param float4 c\n" GUARD "leaked 0\n"},
	{"struct A { float a; int b; };\nstruct B { float4 c : COLOR; };",
	 "struct A { float a; int b; }\n"
	 "struct B { float4 c : COLOR; }\n" GUARD "leaked 0\n"},
	{"struct E { };", "struct E { }\n" GUARD "leaked 0\n"},
	{"struct E {};", "struct E { }\n" GUARD "leaked 0\n"},
	{"struct S { float a; float a; };", "struct S { float a; float a; }\n" GUARD "leaked 0\n"},
	{"struct S\n{\n\tfloat4 pos : POSITION;\n\tfloat2 uv  : TEXCOORD0;\n};",
	 "struct S { float4 pos : POSITION; float2 uv : TEXCOORD0; }\n" GUARD "leaked 0\n"},
};

static void structs(void **state)
{
	(void)state;
	CHECK_CASES(struct_cases);
}

static const struct block_case struct_error_cases[] = {
	{"struct S { float a;; float b; };", "struct S { float a; float b; }\n" GUARD "leaked 0\n"},
	/* A missing name skips to the ';' inside the body, so its '}' is read
	 * at the top level (characterized, not endorsed). */
	{"struct { float a; };\nuniform float c;", "param float c\n" GUARD "error 2:8 0 Expected 'name'\n"
						   "error 2:19 0 Expected 'type'\n"
						   "leaked 0\n"},
	/* The skip after a bad name passes the braced body whole, then steps
	 * over the ';' after it to the next one, so the param goes too
	 * (characterized, not endorsed). */
	{"struct 1 { float a; };\nuniform float c;", GUARD "error 2:8 0 Expected 'name'\n"
							   "leaked 0\n"},
	{"struct S float a; };\nuniform float c;", "param float c\n" GUARD "error 2:17 0 Expected '{'\n"
						   "error 2:19 0 Expected 'type'\n"
						   "leaked 0\n"},
	{"struct S { 1 a; float b; };", "struct S { float b; }\n" GUARD "error 2:12 0 Expected 'type name'\n"
					"leaked 0\n"},
	{"struct S { float ; float b; };", "struct S { float b; }\n" GUARD "error 2:18 0 Syntax error\n"
					   "leaked 0\n"},
	{"struct S { float }; uniform float c;", "struct S { }\n"
						 "param float c\n" GUARD "error 2:18 0 Syntax error\n"
						 "leaked 0\n"},
	{"struct S { float 1; float b; };", "struct S { float b; }\n" GUARD "error 2:18 0 Expected 'variable name'\n"
					    "leaked 0\n"},
	{"struct S { float a : ; float b; };", "struct S { float b; }\n" GUARD "error 2:22 0 Syntax error\n"
					       "leaked 0\n"},
	{"struct S { float a : }; uniform float c;", "struct S { }\n"
						     "param float c\n" GUARD "error 2:22 0 Syntax error\n"
						     "leaked 0\n"},
	{"struct S { float a : 1; float b; };", "struct S { float b; }\n" GUARD "error 2:22 0 Expected 'mapping name'\n"
						"leaked 0\n"},
	{"struct S { float : POSITION; float b; };",
	 "struct S { float b; }\n" GUARD "error 2:18 0 Expected 'variable name'\n"
	 "leaked 0\n"},
	/* A stray token before ';' drops the variable with no error
	 * (characterized, not endorsed). */
	{"struct S { float a b; float c; };", "struct S { float c; }\n" GUARD "leaked 0\n"},
	/* The same after a mapping (characterized, not endorsed). */
	{"struct S { float a : POSITION b; float c; };", "struct S { float c; }\n" GUARD "leaked 0\n"},
	/* A skip that stops at '}' is not taken as the end of the struct: it
	 * reads on to the end of the text and is dropped with the param
	 * (characterized, not endorsed). */
	{"struct S { float a b }; uniform float c;", GUARD "error 2:41 0 Unexpected EOF\n"
							   "leaked 0\n"},
	{"struct S { float a; }\nuniform float c;", "param float c\n" GUARD "error 3:1 0 Expected ';'\n"
						    "leaked 0\n"},
	{"struct S { float a; } x;\nuniform float c;", GUARD "error 2:23 0 Expected ';'\n"
							     "error 2:24 0 Expected 'name'\n"
							     "leaked 0\n"},
	/* A stray '}' skips to the next ';' past the struct's braces, so the
	 * struct is lost (characterized, not endorsed). */
	{"}\nstruct S { float a; };", GUARD "error 2:1 0 Expected 'type'\n"
					    "error 3:23 0 Unexpected EOF\n"
					    "leaked 0\n"},
};

static void struct_errors(void **state)
{
	(void)state;
	CHECK_CASES(struct_error_cases);
}

static const struct block_case struct_eof_cases[] = {
	{"struct", GUARD "error 2:7 0 Unexpected EOF\n"
			 "leaked 0\n"},
	{"struct S", GUARD "error 2:9 0 Unexpected EOF\n"
			   "leaked 0\n"},
	{"struct S {", GUARD "error 2:11 0 Unexpected EOF\n"
			     "leaked 0\n"},
	{"struct S { float", GUARD "error 2:17 0 Unexpected EOF\n"
				   "leaked 0\n"},
	{"struct S { float a", GUARD "error 2:19 0 Unexpected EOF\n"
				     "leaked 0\n"},
	{"struct S { float a :", GUARD "error 2:21 0 Unexpected EOF\n"
				       "leaked 0\n"},
	{"struct S { float a : POSITION", GUARD "error 2:30 0 Unexpected EOF\n"
						"leaked 0\n"},
	{"struct S { float a : POSITION b", GUARD "error 2:32 0 Unexpected EOF\n"
						  "leaked 0\n"},
	{"struct S { float a;", GUARD "error 2:20 0 Unexpected EOF\n"
				      "leaked 0\n"},
	{"struct S { float a; }", GUARD "error 2:22 0 Unexpected EOF\n"
					"leaked 0\n"},
};

static void struct_eof(void **state)
{
	(void)state;
	CHECK_CASES(struct_eof_cases);
}

/* ------------------------------------------------------------------------ */

static const struct block_case sampler_cases[] = {
	{"sampler_state def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; };\nuniform float4 c;",
	 "sampler def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; }\n"
	 "param float4 c\n" GUARD "leaked 0\n"},
	{"sampler_state a { Filter = Point; };\nsampler_state b { MaxAnisotropy = 4; BorderColor = 00ff00ff; };",
	 "sampler a { Filter = Point; }\n"
	 "sampler b { MaxAnisotropy = 4; BorderColor = 00ff00ff; }\n" GUARD "leaked 0\n"},
	{"sampler_state s { };", "sampler s { }\n" GUARD "leaked 0\n"},
	{"sampler_state s {};", "sampler s { }\n" GUARD "leaked 0\n"},
	{"sampler_state s { Filter = Linear; Filter = Point; };",
	 "sampler s { Filter = Linear; Filter = Point; }\n" GUARD "leaked 0\n"},
	/* A value's tokens are joined without their spaces. */
	{"sampler_state s { BorderColor = 0 0 0 1; };", "sampler s { BorderColor = 0001; }\n" GUARD "leaked 0\n"},
	{"sampler_state s\n{\n\tFilter   = Linear;\n\tAddressU = Clamp;\n};",
	 "sampler s { Filter = Linear; AddressU = Clamp; }\n" GUARD "leaked 0\n"},
};

static void samplers(void **state)
{
	(void)state;
	CHECK_CASES(sampler_cases);
}

static const struct block_case sampler_error_cases[] = {
	{"sampler_state s { Filter Linear; AddressU = Wrap; };",
	 "sampler s { AddressU = Wrap; }\n" GUARD "error 2:32 0 Expected '='\n"
	 "leaked 0\n"},
	/* A missing ';' joins the next item into the value, with no error
	 * (characterized, not endorsed). */
	{"sampler_state s { Filter = Linear AddressU = Wrap; };",
	 "sampler s { Filter = LinearAddressU=Wrap; }\n" GUARD "leaked 0\n"},
	/* An empty value drops the item with no error (characterized, not
	 * endorsed). Its state name is freed with it (#3550). */
	{"sampler_state s { Filter = ; AddressU = Wrap; };", "sampler s { AddressU = Wrap; }\n" GUARD "leaked 0\n"},
	{"sampler_state s { Filter = ; };", "sampler s { }\n" GUARD "leaked 0\n"},
	{"sampler_state s { 1 = Linear; AddressU = Wrap; };",
	 "sampler s { AddressU = Wrap; }\n" GUARD "error 2:19 0 Expected 'state name'\n"
	 "leaked 0\n"},
	/* The skip after a missing state name steps over the ';' it starts
	 * on, so the next item goes too (characterized, not endorsed). */
	{"sampler_state s { ; AddressU = Wrap; };", "sampler s { }\n" GUARD "error 2:19 0 Expected 'state name'\n"
						    "leaked 0\n"},
	{"sampler_state { Filter = Linear; };\nuniform float c;",
	 "param float c\n" GUARD "error 2:15 0 Expected 'name'\n"
	 "error 2:34 0 Expected 'type'\n"
	 "leaked 0\n"},
	{"sampler_state 1 { Filter = Linear; };\nuniform float c;", GUARD "error 2:15 0 Expected 'name'\n"
									  "leaked 0\n"},
	{"sampler_state s Filter = Linear; };\nuniform float c;", "param float c\n" GUARD "error 2:32 0 Expected '{'\n"
								  "error 2:34 0 Expected 'type'\n"
								  "leaked 0\n"},
	{"sampler_state s { Filter = Linear; }\nuniform float c;", "param float c\n" GUARD "error 3:1 0 Expected ';'\n"
								   "leaked 0\n"},
	{"sampler_state s { Filter = Linear; } x;\nuniform float c;", GUARD "error 2:38 0 Expected ';'\n"
									    "error 2:39 0 Expected 'name'\n"
									    "leaked 0\n"},
	/* A missing ';' reads the value on past the '}', so the block runs to
	 * the end of the text and is dropped with the param (characterized,
	 * not endorsed). */
	{"sampler_state s { Filter = Linear }; uniform float c;", GUARD "error 2:53 0 Expected '='\n"
									"error 2:53 0 Unexpected EOF\n"
									"leaked 0\n"},
	/* A stray '}' skips to the next ';' past the sampler's braces, so the
	 * sampler is lost (characterized, not endorsed). */
	{"}\nsampler_state s { Filter = Linear; };", GUARD "error 2:1 0 Expected 'type'\n"
							   "error 3:38 0 Unexpected EOF\n"
							   "leaked 0\n"},
};

static void sampler_errors(void **state)
{
	(void)state;
	CHECK_CASES(sampler_error_cases);
}

static const struct block_case sampler_eof_cases[] = {
	{"sampler_state", GUARD "error 2:14 0 Unexpected EOF\n"
				"leaked 0\n"},
	{"sampler_state s", GUARD "error 2:16 0 Unexpected EOF\n"
				  "leaked 0\n"},
	/* End of text after '{' leaves the parser on the '{', which is then
	 * read as a code segment (characterized, not endorsed). */
	{"sampler_state s {", GUARD "error 2:17 0 Unexpected EOF\n"
				    "error 2:17 0 Unexpected code segment\n"
				    "leaked 0\n"},
	{"sampler_state s { Filter", GUARD "error 2:25 0 Unexpected EOF\n"
					   "leaked 0\n"},
	/* End of text inside an item frees its state name and the value read
	 * so far (#3550). */
	{"sampler_state s { Filter =", GUARD "error 2:27 0 Unexpected EOF\n"
					     "leaked 0\n"},
	{"sampler_state s { Filter = Linear", GUARD "error 2:34 0 Unexpected EOF\n"
						    "leaked 0\n"},
	{"sampler_state s { Filter = Linear Point", GUARD "error 2:40 0 Unexpected EOF\n"
							  "leaked 0\n"},
	{"sampler_state s { Filter = Linear;", GUARD "error 2:34 0 Unexpected EOF\n"
						     "leaked 0\n"},
	{"sampler_state s { Filter = Linear; }", GUARD "error 2:37 0 Unexpected EOF\n"
						       "leaked 0\n"},
};

static void sampler_eof(void **state)
{
	(void)state;
	CHECK_CASES(sampler_eof_cases);
}

/* ------------------------------------------------------------------------ */

static const struct block_case mixed_cases[] = {
	{"struct VertInOut { float4 pos : POSITION; };\nsampler_state def_sampler { Filter = Linear; };\nuniform float4x4 ViewProj;",

	 "struct VertInOut { float4 pos : POSITION; }\n"
	 "sampler def_sampler { Filter = Linear; }\n"
	 "param float4x4 ViewProj\n" GUARD "leaked 0\n"},
	{"sampler_state s { Filter = ; };\nstruct S { float a; };\nsampler_state t { AddressU = Wrap; };",
	 "struct S { float a; }\n"
	 "sampler s { }\n"
	 "sampler t { AddressU = Wrap; }\n" GUARD "leaked 0\n"},
};

static void mixed(void **state)
{
	(void)state;
	CHECK_CASES(mixed_cases);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(structs),  cmocka_unit_test(struct_errors),  cmocka_unit_test(struct_eof),
		cmocka_unit_test(samplers), cmocka_unit_test(sampler_errors), cmocka_unit_test(sampler_eof),
		cmocka_unit_test(mixed),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
