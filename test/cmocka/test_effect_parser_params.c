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
 * Characterization of the effect parser's parameter value assignments
 * (#3610): ep_parse() on an effect's text, then the params it kept, written
 * out one line each, and compared whole:
 *
 *   param [const] [uniform] [property] <type> <name> [[<count>]] [= {<bytes>}]
 *   func <return type> <name>
 *   error <row>:<column> <level> <text>
 *   leaked <allocations>
 *
 * Params in the order the parser pushed them, with their flags, their
 * array_count when it is not 0, and every byte of default_val in hex when it
 * has any (an int or a float is its 4 bytes, little-endian: 1.0f is
 * 00 00 80 3f; a string or a texture path is its bytes and the terminating
 * zero). Then the functions (where the parser went on after a param), every
 * error in the order it was added (level 1 is a warning), and the
 * allocations still held after ep_free(). They pin the current C behaviour;
 * cases marked "characterized, not endorsed" are odd but real.
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

struct param_case {
	const char *text;
	const char *expected;
};

static char dump_buf[8192];
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

static void dump_param(const struct ep_param *param)
{
	if (param->is_const)
		dump_cat(" const");
	if (param->is_uniform)
		dump_cat(" uniform");
	if (param->is_property)
		dump_cat(" property");
	dump_cat(" %s %s", str_or_null(param->type), str_or_null(param->name));
	if (param->array_count)
		dump_cat(" [%d]", param->array_count);
	if (param->default_val.num) {
		dump_cat(" = {");
		for (size_t i = 0; i < param->default_val.num; i++)
			dump_cat(i ? " %02x" : "%02x", param->default_val.array[i]);
		dump_cat("}");
	}
	if (param->properties.num)
		dump_cat(" (%zu properties)", param->properties.num);
	if (param->annotations.num) {
		dump_cat(" <");
		for (size_t i = 0; i < param->annotations.num; i++) {
			dump_param(param->annotations.array + i);
			dump_cat(";");
		}
		dump_cat(" >");
	}
}

static void dump_parser(struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->params.num; i++) {
		dump_cat("param");
		dump_param(ep->params.array + i);
		dump_cat("\n");
	}
	for (size_t i = 0; i < ep->funcs.num; i++)
		dump_cat("func %s %s\n", str_or_null(ep->funcs.array[i].ret_type),
			 str_or_null(ep->funcs.array[i].name));
	for (size_t i = 0; i < ep->cfp.error_list.errors.num; i++) {
		struct error_item *err = ep->cfp.error_list.errors.array + i;

		dump_cat("error %u:%u %d %s\n", (unsigned)err->row, (unsigned)err->column, err->level,
			 str_or_null(err->error));
	}
}

static void check_cases(const struct param_case *cases, size_t count)
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
		ok = ep_parse(&ep, NULL, text, "test_effect_parser_params");
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

static const struct param_case scalar_cases[] = {
	{"uniform float f;", "param uniform float f\n" GUARD "leaked 0\n"},
	{"const float f; property float g; float h;", "param const float f\n"
						      "param property float g\n"
						      "param float h\n" GUARD "leaked 0\n"},
	{"uniform int i = 3;", "param uniform int i = {03 00 00 00}\n" GUARD "leaked 0\n"},
	{"uniform int i = -3;", "param uniform int i = {fd ff ff ff}\n" GUARD "leaked 0\n"},
	{"uniform int i = - 3;", "param uniform int i = {fd ff ff ff}\n" GUARD "leaked 0\n"},
	/* An int reads the leading digits of the number token. */
	{"uniform int i = 1.5;", "param uniform int i = {01 00 00 00}\n" GUARD "leaked 0\n"},
	{"uniform float f = 1.5;", "param uniform float f = {00 00 c0 3f}\n" GUARD "leaked 0\n"},
	{"uniform float f = -1.5;", "param uniform float f = {00 00 c0 bf}\n" GUARD "leaked 0\n"},
	{"uniform float f = - 1.5;", "param uniform float f = {00 00 c0 bf}\n" GUARD "leaked 0\n"},
	{"uniform float f = 2;", "param uniform float f = {00 00 00 40}\n" GUARD "leaked 0\n"},
	/* A float reads as far as os_strtod() goes. */
	{"uniform float f = 1.5f;", "param uniform float f = {00 00 c0 3f}\n" GUARD "leaked 0\n"},
	/* A value that is not a number is reported at it, and the skip to
	 * the ';' drops the param. */
	{"uniform float f = x; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:19 0 Expected 'numeric value'\n"
	 "leaked 0\n"},
	/* Only one '-' is taken. */
	{"uniform float f = --1.0; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:20 0 Expected 'numeric value'\n"
	 "leaked 0\n"},
	{"uniform bool b = true;", "param uniform bool b = {01 00 00 00}\n" GUARD "leaked 0\n"},
	{"uniform bool b = false;", "param uniform bool b = {00 00 00 00}\n" GUARD "leaked 0\n"},
	/* A bool other than true or false does not skip to the ';': the
	 * statement after it starts at the bad value and reports it again
	 * (characterized, not endorsed). */
	{"uniform bool b = 1; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:18 0 Expected 'true or false'\n"
	 "error 2:18 0 Expected 'type'\n"
	 "leaked 0\n"},
	{"uniform string s = \"hi\";", "param uniform string s = {68 69 00}\n" GUARD "leaked 0\n"},
	{"uniform string s = \"a\\\"b\\n\";", "param uniform string s = {61 22 62 0a 00}\n" GUARD "leaked 0\n"},
	{"uniform string s = \"\";", "param uniform string s = {00}\n" GUARD "leaked 0\n"},
	{"uniform string s = hi; uniform float g;", "param uniform float g\n" GUARD "error 2:20 0 Expected 'string'\n"
						    "leaked 0\n"},
	{"uniform texture2d image;", "param uniform texture2d image\n" GUARD "leaked 0\n"},
	{"uniform texture2d image = \"a.png\";",
	 "param uniform texture2d image = {61 2e 70 6e 67 00}\n" GUARD "leaked 0\n"},
	{"uniform texture_rect image = \"a.png\";",
	 "param uniform texture_rect image = {61 2e 70 6e 67 00}\n" GUARD "leaked 0\n"},
	{"uniform texture2d image = a; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:27 0 Expected 'texture path string'\n"
	 "leaked 0\n"},
	/* An invalid type is reported at the '=', with no skip, so the
	 * statement after it starts at the '=' (characterized, not endorsed). */
	{"uniform foo v = 1; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:15 0 Invalid type 'foo' used for assignment\n"
	 "error 2:15 0 Expected 'type'\n"
	 "leaked 0\n"},
	/* No ';' after the value drops the param with no error, and the
	 * parser goes on from the token after the value (characterized, not
	 * endorsed). */
	{"uniform float f = 1.0 uniform float g;", "param uniform float g\n" GUARD "leaked 0\n"},
	/* Anything else after the name drops the param with no error of its
	 * own (characterized, not endorsed). */
	{"uniform float f 1.0; uniform float g;", "param uniform float g\n" GUARD "error 2:17 0 Expected 'type'\n"
						  "leaked 0\n"},
};

static void scalars(void **state)
{
	(void)state;
	CHECK_CASES(scalar_cases);
}

static const struct param_case vector_cases[] = {
	{"uniform float4 c = {1.0, 0.5, 0.25, 1.0};",
	 "param uniform float4 c = {00 00 80 3f 00 00 00 3f 00 00 80 3e 00 00 80 3f}\n" GUARD "leaked 0\n"},
	{"uniform float4 c = float4(1.0, 0.5, 0.25, 1.0);",
	 "param uniform float4 c = {00 00 80 3f 00 00 00 3f 00 00 80 3e 00 00 80 3f}\n" GUARD "leaked 0\n"},
	{"uniform float2 c = {-1.0, - 2.0};",
	 "param uniform float2 c = {00 00 80 bf 00 00 00 c0}\n" GUARD "leaked 0\n"},
	{"uniform int2 v = {1, -2};", "param uniform int2 v = {01 00 00 00 fe ff ff ff}\n" GUARD "leaked 0\n"},
	{"uniform int3 v = int3(1, 2, 3);",
	 "param uniform int3 v = {01 00 00 00 02 00 00 00 03 00 00 00}\n" GUARD "leaked 0\n"},
	{"uniform float3x3 m = {1, 2, 3, 4, 5, 6, 7, 8, 9};",
	 "param uniform float3x3 m = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40 00 00 a0 40 00 00 c0 40 00 00 e0 40 00 00 00 41 00 00 10 41}\n" GUARD
	 "leaked 0\n"},
	{"uniform float4x4 m = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};",
	 "param uniform float4x4 m = {00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f}\n" GUARD
	 "leaked 0\n"},
	{"uniform int2x2 m = {1, 2, 3, 4};",
	 "param uniform int2x2 m = {01 00 00 00 02 00 00 00 03 00 00 00 04 00 00 00}\n" GUARD "leaked 0\n"},
	{"uniform float1 f = {1.0};", "param uniform float1 f = {00 00 80 3f}\n" GUARD "leaked 0\n"},
	/* A bad row count is reported at the '=', and the values are read
	 * anyway, by the row digit (characterized, not endorsed). */
	{"uniform float5 v = {1, 2, 3, 4, 5};",
	 "param uniform float5 v = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40 00 00 a0 40}\n" GUARD
	 "error 2:18 0 Invalid row count\n"
	 "leaked 0\n"},
	/* No values are read for a row count of 0, so the '}' is taken where
	 * the ';' should be (characterized, not endorsed). */
	{"uniform float0 v = {}; uniform float g;", "param uniform float g\n" GUARD "error 2:18 0 Invalid row count\n"
						    "error 2:21 0 Expected 'type'\n"
						    "leaked 0\n"},
	/* A missing ',' or end is reported at the ';' the skip goes to. */
	{"uniform float4x5 m = {1, 2}; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:20 0 Invalid column count\n"
	 "error 2:28 0 Expected ','\n"
	 "leaked 0\n"},
	{"uniform float4 c = {1.0, 0.5}; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:30 0 Expected ','\n"
	 "leaked 0\n"},
	{"uniform float2 c = {1.0 0.5}; uniform float g;", "param uniform float g\n" GUARD "error 2:29 0 Expected ','\n"
							   "leaked 0\n"},
	{"uniform float2 c = {1.0, 0.5, 0.25}; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:36 0 Expected '}'\n"
	 "leaked 0\n"},
	{"uniform float2 c = {1.0, x}; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:26 0 Expected 'numeric value'\n"
	 "leaked 0\n"},
	/* A vector without '{' or its own type skips to the ';', and reports
	 * the '{' there. */
	{"uniform float2 c = 1.0; uniform float g;", "param uniform float g\n" GUARD "error 2:23 0 Expected '{'\n"
						     "leaked 0\n"},
	{"uniform float2 c = float3(1.0, 2.0); uniform float g;",
	 "param uniform float g\n" GUARD "error 2:36 0 Expected '{'\n"
	 "leaked 0\n"},
	{"uniform float2 c = float2 1.0, 2.0); uniform float g;",
	 "param uniform float g\n" GUARD "error 2:36 0 Expected '('\n"
	 "leaked 0\n"},
	{"uniform float2 c = float2(1.0, 2.0}; uniform float g;",
	 "param uniform float g\n" GUARD "error 2:36 0 Expected ')'\n"
	 "leaked 0\n"},
	{"uniform float2 c = {1.0, 2.0); uniform float g;",
	 "param uniform float g\n" GUARD "error 2:30 0 Expected '}'\n"
	 "leaked 0\n"},
};

static void vectors(void **state)
{
	(void)state;
	CHECK_CASES(vector_cases);
}

static const struct param_case array_cases[] = {
	{"uniform float a[3];", "param uniform float a [3]\n" GUARD "leaked 0\n"},
	{"uniform float a[3] = 1.0;", "param uniform float a [3] = {00 00 80 3f}\n" GUARD "leaked 0\n"},
	/* A count of 0 is no array. */
	{"uniform float a[0];", "param uniform float a\n" GUARD "leaked 0\n"},
	/* A count that is not a number token drops the param with no error of
	 * its own; the statement after it starts at the bad token
	 * (characterized, not endorsed). */
	{"uniform float a[-1]; uniform float g;", "param uniform float g\n" GUARD "error 2:17 0 Expected 'type'\n"
						  "leaked 0\n"},
	{"uniform float a[x]; uniform float g;", "param uniform float g\n" GUARD "error 2:18 0 Expected 'name'\n"
						 "leaked 0\n"},
	{"uniform float a[1.5]; uniform float g;", "param uniform float g\n" GUARD "error 2:17 0 Expected 'type'\n"
						   "leaked 0\n"},
	/* A missing ']' skips from the ';' it found to the next one, so the
	 * statement after it goes too (characterized, not endorsed). */
	{"uniform float a[3; uniform float g;", GUARD "error 2:35 0 Expected ']'\n"
						      "error 2:36 0 Unexpected EOF\n"
						      "leaked 0\n"},
	{"uniform float a[3] uniform float g;", "param uniform float g\n" GUARD "leaked 0\n"},
	{"uniform float a[]; uniform float g;", "param uniform float g\n" GUARD "error 2:17 0 Expected 'type'\n"
						"leaked 0\n"},
	{"uniform float4 a[2] = {1, 2, 3, 4};",
	 "param uniform float4 a [2] = {00 00 80 3f 00 00 00 40 00 00 40 40 00 00 80 40}\n" GUARD "leaked 0\n"},
};

static void arrays(void **state)
{
	(void)state;
	CHECK_CASES(array_cases);
}

static const struct param_case eof_cases[] = {
	{"uniform float f", GUARD "error 2:16 0 Unexpected EOF\n"
				  "leaked 0\n"},
	{"uniform float f =", GUARD "error 2:18 0 Unexpected EOF\n"
				    "leaked 0\n"},
	/* End of text after a '-' adds no error (characterized, not
	 * endorsed). */
	{"uniform float f = -", GUARD "leaked 0\n"},
	{"uniform float f = 1.0", GUARD "error 2:22 0 Unexpected EOF\n"
					"leaked 0\n"},
	{"uniform bool b =", GUARD "error 2:17 0 Unexpected EOF\n"
				   "leaked 0\n"},
	{"uniform string s =", GUARD "error 2:19 0 Unexpected EOF\n"
				     "leaked 0\n"},
	{"uniform texture2d image =", GUARD "error 2:26 0 Unexpected EOF\n"
					    "leaked 0\n"},
	{"uniform float2 c =", GUARD "error 2:19 0 Unexpected EOF\n"
				     "leaked 0\n"},
	{"uniform float2 c = {", GUARD "error 2:21 0 Unexpected EOF\n"
				       "leaked 0\n"},
	{"uniform float2 c = {1.0", GUARD "error 2:24 0 Unexpected EOF\n"
					  "leaked 0\n"},
	{"uniform float2 c = {1.0,", GUARD "error 2:25 0 Unexpected EOF\n"
					   "leaked 0\n"},
	{"uniform float2 c = {1.0, 2.0", GUARD "error 2:29 0 Unexpected EOF\n"
					       "leaked 0\n"},
	{"uniform float2 c = {1.0, 2.0}", GUARD "error 2:30 0 Unexpected EOF\n"
						"leaked 0\n"},
	{"uniform float2 c = float2", GUARD "error 2:26 0 Unexpected EOF\n"
					    "leaked 0\n"},
	{"uniform float2 c = float2(", GUARD "error 2:27 0 Unexpected EOF\n"
					     "leaked 0\n"},
	/* End of text in the skip after a vector without '{' adds no error
	 * (characterized, not endorsed). */
	{"uniform float2 c = 1.0", GUARD "leaked 0\n"},
	{"uniform float a[", GUARD "error 2:17 0 Unexpected EOF\n"
				   "leaked 0\n"},
	{"uniform float a[3", GUARD "error 2:18 0 Unexpected EOF\n"
				    "leaked 0\n"},
	{"uniform float a[3]", GUARD "error 2:19 0 Unexpected EOF\n"
				     "leaked 0\n"},
};

static void eof(void **state)
{
	(void)state;
	CHECK_CASES(eof_cases);
}

/* ------------------------------------------------------------------------ */

static const struct param_case mixed_cases[] = {
	{"uniform float4x4 ViewProj;\n"
	 "uniform texture2d image;\n"
	 "uniform float4 color = {1.0, 1.0, 1.0, 1.0};\n"
	 "float4 PSDraw(float4 v) { return v * color; }\n"
	 "uniform float alpha = 0.5;",
	 "param uniform float4x4 ViewProj\n"
	 "param uniform texture2d image\n"
	 "param uniform float4 color = {00 00 80 3f 00 00 80 3f 00 00 80 3f 00 00 80 3f}\n"
	 "param uniform float alpha = {00 00 00 3f}\n"
	 "func float4 PSDraw\n" GUARD "leaked 0\n"},
};

static void mixed(void **state)
{
	(void)state;
	CHECK_CASES(mixed_cases);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(scalars), cmocka_unit_test(vectors), cmocka_unit_test(arrays),
		cmocka_unit_test(eof),     cmocka_unit_test(mixed),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
