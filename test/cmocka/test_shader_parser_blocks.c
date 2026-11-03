#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/shader-parser.h>

/*
 * Characterization of the shader parser's sampler-state and struct blocks
 * through the real libobs (#3526): shader_parse() on a shader's text, then
 * what it kept, written out one line each, and compared whole:
 *
 *   ok <shader_parse()'s result>
 *   sampler <name> { <state> = <value>; ... }
 *   struct <name> { <type> <name>[ : <mapping>]; ... }
 *   param <type> <name>
 *   error <row>:<column> <level> <text>
 *
 * Samplers, structs and params in the order the parser pushed them, then
 * every error in the order it was added. The params show where the parser
 * went on after a block. They pin the current C behaviour; cases marked
 * "characterized, not endorsed" are odd but real. No display and no
 * graphics context, so it runs on every platform.
 */

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

static const char *dump(struct shader_parser *sp, bool ok)
{
	dump_len = 0;
	dump_buf[0] = 0;
	dump_cat("ok %d\n", ok ? 1 : 0);

	for (size_t i = 0; i < sp->samplers.num; i++) {
		struct shader_sampler *ss = sp->samplers.array + i;

		dump_cat("sampler %s {", str_or_null(ss->name));
		for (size_t j = 0; j < ss->states.num; j++)
			dump_cat(" %s = %s;", str_or_null(ss->states.array[j]),
				 j < ss->values.num ? str_or_null(ss->values.array[j]) : "(none)");
		if (ss->values.num != ss->states.num)
			dump_cat(" (%zu values)", ss->values.num);
		dump_cat(" }\n");
	}
	for (size_t i = 0; i < sp->structs.num; i++) {
		struct shader_struct *st = sp->structs.array + i;

		dump_cat("struct %s {", str_or_null(st->name));
		for (size_t j = 0; j < st->vars.num; j++) {
			struct shader_var *var = st->vars.array + j;

			dump_cat(" %s %s", str_or_null(var->type), str_or_null(var->name));
			if (var->mapping)
				dump_cat(" : %s", var->mapping);
			dump_cat(";");
		}
		dump_cat(" }\n");
	}
	for (size_t i = 0; i < sp->params.num; i++)
		dump_cat("param %s %s\n", str_or_null(sp->params.array[i].type), str_or_null(sp->params.array[i].name));
	for (size_t i = 0; i < sp->funcs.num; i++)
		dump_cat("func %s %s\n", str_or_null(sp->funcs.array[i].return_type),
			 str_or_null(sp->funcs.array[i].name));
	for (size_t i = 0; i < sp->cfp.error_list.errors.num; i++) {
		struct error_item *err = sp->cfp.error_list.errors.array + i;

		dump_cat("error %u:%u %d %s\n", (unsigned)err->row, (unsigned)err->column, err->level,
			 str_or_null(err->error));
	}
	return dump_buf;
}

static void check_cases(const struct block_case *cases, size_t count)
{
	size_t failed = 0;

	for (size_t i = 0; i < count; i++) {
		struct shader_parser sp;
		bool ok;
		const char *got;

		shader_parser_init(&sp);
		ok = shader_parse(&sp, cases[i].text, "test_shader_parser_blocks");
		got = dump(&sp, ok);
		if (strcmp(got, cases[i].expected) != 0) {
			fprintf(stderr, "case %zu: %s\n--- expected\n%s--- got\n%s", i, cases[i].text,
				cases[i].expected, got);
			failed++;
		}
		shader_parser_free(&sp);
	}

	if (failed) {
		fprintf(stderr, "%zu of %zu cases differ\n", failed, count);
		fail();
	}
}

#define CHECK_CASES(cases) check_cases(cases, sizeof(cases) / sizeof(cases[0]))

/* ------------------------------------------------------------------------ */

static const struct block_case sampler_state_cases[] = {
	{
		"sampler_state def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; };\nuniform float4 c;",
		"ok 1\n"
		"sampler def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; }\n"
		"param float4 c\n",
	},
	{
		"sampler_state a { Filter = Point; };\nsampler_state b { MaxAnisotropy = 4; BorderColor = 00ff00ff; };",
		"ok 1\n"
		"sampler a { Filter = Point; }\n"
		"sampler b { MaxAnisotropy = 4; BorderColor = 00ff00ff; }\n",
	},
	{
		"sampler_state s { };",
		"ok 1\n"
		"sampler s { }\n",
	},
	{
		"sampler_state s {};",
		"ok 1\n"
		"sampler s { }\n",
	},
	{
		"sampler_state s { Filter = Linear; Filter = Point; };",
		"ok 1\n"
		"sampler s { Filter = Linear; Filter = Point; }\n",
	},
	{
		"sampler_state s { Filter = Linear; };",
		"ok 1\n"
		"sampler s { Filter = Linear; }\n",
	},
};

static void sampler_states(void **state)
{
	(void)state;
	CHECK_CASES(sampler_state_cases);
}

static const struct block_case sampler_state_error_cases[] = {
	{
		"sampler_state s { Filter Linear; AddressU = Wrap; };",
		"ok 0\n"
		"sampler s { AddressU = Wrap; }\n"
		"error 1:32 0 Expected '='\n",
	},
	/* A missing ';' skips to the next one, so the item after it goes too
	 * (characterized, not endorsed). */
	{
		"sampler_state s { Filter = Linear AddressU = Wrap; };",
		"ok 0\n"
		"sampler s { }\n"
		"error 1:50 0 Expected ';'\n",
	},
	/* The ';' is read as the value, so the item's own ';' is missing
	 * (characterized, not endorsed). */
	{
		"sampler_state s { Filter = ; AddressU = Wrap; };",
		"ok 0\n"
		"sampler s { }\n"
		"error 1:45 0 Expected ';'\n",
	},
	{
		"sampler_state s { 1 = Linear; AddressU = Wrap; };",
		"ok 0\n"
		"sampler s { AddressU = Wrap; }\n"
		"error 1:19 0 Expected 'state name'\n",
	},
	/* A bad name skips to a ';', passing over the braces, and the
	 * declaration after the block is lost with it (characterized, not
	 * endorsed). */
	{
		"sampler_state 1 { Filter = Linear; };\nuniform float a;",
		"ok 0\n"
		"error 1:15 0 Expected 'name'\n",
	},
	{
		"sampler_state s Filter = Linear;\nuniform float a;",
		"ok 0\n"
		"param float a\n"
		"error 1:32 0 Expected '{'\n",
	},
	/* No ';' after the block: the sampler is dropped, and the parser goes on
	 * from the token that is there instead. */
	{
		"sampler_state s { Filter = Linear; }\nuniform float a;",
		"ok 0\n"
		"param float a\n"
		"error 2:1 0 Expected ';'\n",
	},
	{
		"sampler_state s { Filter = Linear; } uniform float a;",
		"ok 0\n"
		"param float a\n"
		"error 1:38 0 Expected ';'\n",
	},
};

static void sampler_state_errors(void **state)
{
	(void)state;
	CHECK_CASES(sampler_state_error_cases);
}

/* The text ends inside the block: the sampler is dropped. */
static const struct block_case sampler_state_cut_short_cases[] = {
	{
		"sampler_state",
		"ok 0\n"
		"error 1:14 0 Unexpected EOF\n",
	},
	{
		"sampler_state s",
		"ok 0\n"
		"error 1:16 0 Unexpected EOF\n",
	},
	/* Cut after the '{': shader_parse() then meets the '{' itself
	 * (characterized, not endorsed). */
	{
		"sampler_state s {",
		"ok 0\n"
		"error 1:17 0 Unexpected EOF\n"
		"error 1:17 0 Unexpected code segment\n",
	},
	{
		"sampler_state s { Filter",
		"ok 0\n"
		"error 1:25 0 Unexpected EOF\n",
	},
	{
		"sampler_state s { Filter =",
		"ok 0\n"
		"error 1:27 0 Unexpected EOF\n",
	},
	{
		"sampler_state s { Filter = Linear",
		"ok 0\n"
		"error 1:34 0 Unexpected EOF\n",
	},
	{
		"sampler_state s { Filter = Linear;",
		"ok 0\n"
		"error 1:34 0 Unexpected EOF\n",
	},
	{
		"sampler_state s { Filter = Linear; }",
		"ok 0\n"
		"error 1:37 0 Unexpected EOF\n",
	},
};

static void sampler_state_cut_short(void **state)
{
	(void)state;
	CHECK_CASES(sampler_state_cut_short_cases);
}

static const struct block_case struct_cases[] = {
	{
		"struct VertInOut {\n\tfloat4 pos : POSITION;\n\tfloat2 uv : TEXCOORD0;\n\tfloat4 color;\n};\nuniform float4x4 ViewProj;",
		"ok 1\n"
		"struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; float4 color; }\n"
		"param float4x4 ViewProj\n",
	},
	{
		"struct A { float a; };\nstruct B { int b : X; float4x4 m; };",
		"ok 1\n"
		"struct A { float a; }\n"
		"struct B { int b : X; float4x4 m; }\n",
	},
	{
		"struct S { };",
		"ok 1\n"
		"struct S { }\n",
	},
	{
		"struct S {};",
		"ok 1\n"
		"struct S { }\n",
	},
	{
		"struct S { ; float a; ; };",
		"ok 1\n"
		"struct S { float a; }\n",
	},
	{
		"struct S { float a; };",
		"ok 1\n"
		"struct S { float a; }\n",
	},
};

static void structs(void **state)
{
	(void)state;
	CHECK_CASES(struct_cases);
}

static const struct block_case struct_error_cases[] = {
	/* A bad variable is dropped with its error; the struct keeps the
	 * rest. */
	{
		"struct S { 1 a; float b; };",
		"ok 0\n"
		"struct S { float b; }\n"
		"error 1:12 0 Expected 'type name'\n",
	},
	{
		"struct S { float ; float b; };",
		"ok 0\n"
		"struct S { float b; }\n"
		"error 1:18 0 Syntax error\n",
	},
	{
		"struct S { float }; uniform float a;",
		"ok 0\n"
		"struct S { }\n"
		"param float a\n"
		"error 1:18 0 Syntax error\n",
	},
	{
		"struct S { float a : ; float b; };",
		"ok 0\n"
		"struct S { float b; }\n"
		"error 1:22 0 Syntax error\n",
	},
	{
		"struct S { float a : }; uniform float c;",
		"ok 0\n"
		"struct S { }\n"
		"param float c\n"
		"error 1:22 0 Syntax error\n",
	},
	{
		"struct S { float a : 1; float b; };",
		"ok 0\n"
		"struct S { float b; }\n"
		"error 1:22 0 Expected 'mapping name'\n",
	},
	/* A stray token before the ';' drops the variable with no error
	 * (characterized, not endorsed). */
	{
		"struct S { float a b; float c; };",
		"ok 1\n"
		"struct S { float c; }\n",
	},
	/* A stray token before the '}' skips to it and then steps past it, so
	 * the struct runs on to the end of the text (characterized, not
	 * endorsed). */
	{
		"struct S { float a b }; uniform float c;",
		"ok 0\n"
		"error 1:41 0 Unexpected EOF\n",
	},
	{
		"struct S { float a : POSITION b; float c; };",
		"ok 1\n"
		"struct S { float c; }\n",
	},
	/* As for a sampler state: the declaration after the block is lost
	 * (characterized, not endorsed). */
	{
		"struct 1 { float a; };\nuniform float b;",
		"ok 0\n"
		"error 1:8 0 Expected 'name'\n",
	},
	/* No '{': the parser skips to the ';' and goes on from there, so the
	 * '}' is read as a declaration of its own. */
	{
		"struct S float a; };\nuniform float b;",
		"ok 0\n"
		"param float b\n"
		"error 1:17 0 Expected '{'\n"
		"error 1:19 0 Expected 'type'\n",
	},
	{
		"struct S { float a; }\nuniform float b;",
		"ok 0\n"
		"param float b\n"
		"error 2:1 0 Expected ';'\n",
	},
};

static void struct_errors(void **state)
{
	(void)state;
	CHECK_CASES(struct_error_cases);
}

/* The text ends inside the block: the struct is dropped. */
static const struct block_case struct_cut_short_cases[] = {
	{
		"struct",
		"ok 0\n"
		"error 1:7 0 Unexpected EOF\n",
	},
	{
		"struct S",
		"ok 0\n"
		"error 1:9 0 Unexpected EOF\n",
	},
	{
		"struct S {",
		"ok 0\n"
		"error 1:11 0 Unexpected EOF\n",
	},
	{
		"struct S { float",
		"ok 0\n"
		"error 1:17 0 Unexpected EOF\n",
	},
	{
		"struct S { float a",
		"ok 0\n"
		"error 1:19 0 Unexpected EOF\n",
	},
	{
		"struct S { float a :",
		"ok 0\n"
		"error 1:21 0 Unexpected EOF\n",
	},
	{
		"struct S { float a : POSITION",
		"ok 0\n"
		"error 1:30 0 Unexpected EOF\n",
	},
	{
		"struct S { float a;",
		"ok 0\n"
		"error 1:20 0 Unexpected EOF\n",
	},
	{
		"struct S { float a; }",
		"ok 0\n"
		"error 1:22 0 Unexpected EOF\n",
	},
};

static void struct_cut_short(void **state)
{
	(void)state;
	CHECK_CASES(struct_cut_short_cases);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(sampler_states),          cmocka_unit_test(sampler_state_errors),
		cmocka_unit_test(sampler_state_cut_short), cmocka_unit_test(structs),
		cmocka_unit_test(struct_errors),           cmocka_unit_test(struct_cut_short),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
