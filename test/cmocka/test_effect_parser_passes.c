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
 * Characterization of the effect parser's passes and techniques (#3598):
 * ep_parse() on an effect's text, then what it kept, written out one line
 * each, and compared whole:
 *
 *   technique <name> { pass <name> { vs [<tokens>] ps [<tokens>] } ... }
 *   param <type> <name>
 *   func <return type> <name>
 *   error <row>:<column> <level> <text>
 *   leaked <allocations>
 *
 * Techniques and their passes in the order the parser pushed them, an
 * unnamed pass as "(null)". vs and ps are the pass's vertex_program and
 * fragment_program: every token in the array, each as its text, the end
 * token as <end> when it is the zeroed token (no text, no lexer, type
 * CFTOKEN_NONE). Then the params and functions (where the parser went on
 * after a technique), every error in the order it was added (level 1 is a
 * warning), and the allocations still held after ep_free(). They pin the
 * current C behaviour; cases marked "characterized, not endorsed" are odd
 * but real.
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

struct pass_case {
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

static void dump_tokens(const char *label, const cf_token_array_t *tokens)
{
	dump_cat(" %s [", label);
	for (size_t i = 0; i < tokens->num; i++) {
		const struct cf_token *token = tokens->array + i;

		if (!token->str.array) {
			bool zeroed = token->str.len == 0 && token->lex == NULL && token->type == CFTOKEN_NONE &&
				      token->unmerged_str.array == NULL && token->unmerged_str.len == 0;
			dump_cat(i ? " %s" : "%s", zeroed ? "<end>" : "<end, not zeroed>");
		} else {
			dump_cat(i ? " %.*s" : "%.*s", (int)token->str.len, token->str.array);
		}
	}
	dump_cat("]");
}

static void dump_parser(struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->techniques.num; i++) {
		struct ep_technique *ept = ep->techniques.array + i;

		dump_cat("technique %s {", str_or_null(ept->name));
		for (size_t j = 0; j < ept->passes.num; j++) {
			struct ep_pass *pass = ept->passes.array + j;

			dump_cat(" pass %s {", str_or_null(pass->name));
			dump_tokens("vs", &pass->vertex_program);
			dump_tokens("ps", &pass->fragment_program);
			if (pass->pass)
				dump_cat(" (compiled)");
			dump_cat(" }");
		}
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

static void check_cases(const struct pass_case *cases, size_t count)
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
		ok = ep_parse(&ep, NULL, text, "test_effect_parser_passes");
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

static const struct pass_case technique_cases[] = {
	{"technique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSDrawBare(vert_in); } }",
	 "technique Draw { pass (null) { vs [VSDefault ( vert_in ) ; <end>] ps [PSDrawBare ( vert_in ) ; <end>] } }\n" GUARD
	 "leaked 0\n"},
	{"technique T { pass P0 { vertex_program = VS(v); pixel_program = PS(v); } }",
	 "technique T { pass P0 { vs [VS ( v ) ; <end>] ps [PS ( v ) ; <end>] } }\n" GUARD "leaked 0\n"},
	{"technique T { pass P0 { vertex_shader = VS(v); } pass P1 { pixel_shader = PS(v); } }\n"
	 "technique U { pass { pixel_program = PS2(v, 1.0); vertex_program = VS2(v); } }\n"
	 "uniform float c;",
	 "technique T { pass P0 { vs [VS ( v ) ; <end>] ps [] } pass P1 { vs [] ps [PS ( v ) ; <end>] } }\n"
	 "technique U { pass (null) { vs [VS2 ( v ) ; <end>] ps [PS2 ( v , 1.0 ) ; <end>] } }\n"
	 "param float c\n" GUARD "leaked 0\n"},
	/* A second command for the same program is appended after the first
	 * one's end token (characterized, not endorsed). */
	{"technique T { pass { vertex_shader = VS(v); vertex_shader = VS2(v); } }",
	 "technique T { pass (null) { vs [VS ( v ) ; <end> VS2 ( v ) ; <end>] ps [] } }\n" GUARD "leaked 0\n"},
	{"technique T { pass { } }", "technique T { pass (null) { vs [] ps [] } }\n" GUARD "leaked 0\n"},
	{"technique T { pass {} }", "technique T { pass (null) { vs [] ps [] } }\n" GUARD "leaked 0\n"},
	{"technique T { }", "technique T { }\n" GUARD "leaked 0\n"},
	{"technique T {}\n"
	 "uniform float c;",
	 "technique T { }\n"
	 "param float c\n" GUARD "leaked 0\n"},
	{"technique Draw\n"
	 "{\n"
	 "\tpass\n"
	 "\t{\n"
	 "\t\tvertex_shader = VSDefault(vert_in);\n"
	 "\t\tpixel_shader  = PSDrawBare(vert_in);\n"
	 "\t}\n"
	 "}",
	 "technique Draw { pass (null) { vs [VSDefault ( vert_in ) ; <end>] ps [PSDrawBare ( vert_in ) ; <end>] } }\n" GUARD
	 "leaked 0\n"},
	/* "compile" is skipped with a warning. */
	{"technique T { pass { vertex_shader = compile VS(v); pixel_shader = compile PS(v); } }",
	 "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [PS ( v ) ; <end>] } }\n" GUARD
	 "error 2:38 1 compile keyword not necessary\n"
	 "error 2:68 1 compile keyword not necessary\n"
	 "leaked 0\n"},
	/* A pass name is any token. */
	{"technique T { pass 1 { vertex_shader = VS(v); } }",
	 "technique T { pass 1 { vs [VS ( v ) ; <end>] ps [] } }\n" GUARD "leaked 0\n"},
	/* A call is every token up to ';', whatever it is. */
	{"technique T { pass { vertex_shader = 1 + { x ); } }",
	 "technique T { pass (null) { vs [1 + { x ) ; <end>] ps [] } }\n" GUARD "leaked 0\n"},
	/* An empty call keeps its ';' and the end token (characterized, not
	 * endorsed). */
	{"technique T { pass { vertex_shader = ; } }",
	 "technique T { pass (null) { vs [; <end>] ps [] } }\n" GUARD "leaked 0\n"},
};

static void techniques(void **state)
{
	(void)state;
	CHECK_CASES(technique_cases);
}

static const struct pass_case pass_error_cases[] = {
	/* A missing ';' before the pass's '}' stops the call there, but that '}'
	 * is not taken as the end of the pass: the technique's is, so the
	 * technique reads on into the param and is dropped with it, with no
	 * error for the end of the text (characterized, not endorsed). */
	{"technique T { pass { vertex_shader = VS(v) } }\n"
	 "uniform float c;",
	 GUARD "error 2:44 0 Expected ';'\n"
	       "leaked 0\n"},
	/* The same with a second pass: its tokens are read as commands of the
	 * first one (characterized, not endorsed). */
	{"technique T { pass { vertex_shader = VS(v) } pass Q { pixel_shader = PS(v); } }\n"
	 "uniform float c;",
	 GUARD "error 2:44 0 Expected ';'\n"
	       "error 2:46 0 Syntax error\n"
	       "error 3:1 0 Syntax error\n"
	       "error 3:16 0 Unexpected EOF\n"
	       "leaked 0\n"},
	{"technique T { pass { geometry_shader = GS(v); vertex_shader = VS(v); } }",
	 "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\n" GUARD "error 2:22 0 Syntax error\n"
	 "leaked 0\n"},
	/* An unknown command skips to the next ';' or '}'; stopping on the '}'
	 * is as above (characterized, not endorsed). */
	{"technique T { pass { geometry_shader = GS(v) } }\n"
	 "uniform float c;",
	 GUARD "error 2:22 0 Syntax error\n"
	       "leaked 0\n"},
	{"technique T { pass { vertex_shader VS(v); pixel_shader = PS(v); } }",
	 "technique T { pass (null) { vs [] ps [PS ( v ) ; <end>] } }\n" GUARD "error 2:41 0 Expected '='\n"
	 "leaked 0\n"},
	{"technique T { pass { vertex_shader VS(v) } }\n"
	 "uniform float c;",
	 GUARD "error 2:42 0 Expected '='\n"
	       "leaked 0\n"},
	/* A stray ';' is a syntax error, and the skip steps over it to the next
	 * ';', so the command after it goes too (characterized, not endorsed). */
	{"technique T { pass { ; vertex_shader = VS(v); } }",
	 "technique T { pass (null) { vs [] ps [] } }\n" GUARD "error 2:22 0 Syntax error\n"
	 "leaked 0\n"},
	{"technique T { pass { vertex_shader = compile ; } }",
	 "technique T { pass (null) { vs [; <end>] ps [] } }\n" GUARD "error 2:38 1 compile keyword not necessary\n"
	 "leaked 0\n"},
	/* A pass without '{' takes the next token as its name and reads the
	 * rest as commands up to a '}', which ends the pass; the next '}' ends
	 * the technique (characterized, not endorsed). */
	{"technique T { pass vertex_shader = VS(v); } }\n"
	 "uniform float c;",
	 "technique T { pass vertex_shader { vs [] ps [] } }\n"
	 "param float c\n" GUARD "error 2:36 0 Syntax error\n"
	 "leaked 0\n"},
	/* A second name is a syntax error at the '{', and the skip takes the
	 * first command with it (characterized, not endorsed). */
	{"technique T { pass P Q { vertex_shader = VS(v); } }\n"
	 "uniform float c;",
	 "technique T { pass P { vs [] ps [] } }\n"
	 "param float c\n" GUARD "error 2:24 0 Syntax error\n"
	 "leaked 0\n"},
	/* A stray ';' after a pass skips to the technique's '}' with no error
	 * (characterized, not endorsed). */
	{"technique T { pass { vertex_shader = VS(v); }; }\n"
	 "uniform float c;",
	 "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\n"
	 "param float c\n" GUARD "leaked 0\n"},
};

static void pass_errors(void **state)
{
	(void)state;
	CHECK_CASES(pass_error_cases);
}

static const struct pass_case technique_error_cases[] = {
	/* An entry other than a pass skips to the next '}' (past any braces),
	 * and that '}' ends the technique: the passes after it go, with no
	 * error (characterized, not endorsed). */
	{"technique T { foo { } pass { vertex_shader = VS(v); } }\n"
	 "uniform float c;",
	 "technique T { }\n"
	 "param float c\n" GUARD "leaked 0\n"},
	{"technique T { foo; pass { vertex_shader = VS(v); } }\n"
	 "uniform float c;",
	 "technique T { }\n"
	 "param float c\n" GUARD "leaked 0\n"},
	{"technique T { pass { vertex_shader = VS(v); } foo }\n"
	 "uniform float c;",
	 "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\n"
	 "param float c\n" GUARD "leaked 0\n"},
	{"technique T { ; pass { vertex_shader = VS(v); } }\n"
	 "uniform float c;",
	 "technique T { }\n"
	 "param float c\n" GUARD "leaked 0\n"},
	/* A technique without '{' skips to the next ';' (past any braces) and
	 * is dropped. */
	{"technique T pass { vertex_shader = VS(v); } };\n"
	 "uniform float c;",
	 "param float c\n" GUARD "error 2:46 0 Expected '{'\n"
	 "leaked 0\n"},
	/* The skip starts after the ';' it is on, so the next statement goes
	 * too (characterized, not endorsed). */
	{"technique T;\n"
	 "uniform float c;",
	 GUARD "error 3:16 0 Expected '{'\n"
	       "leaked 0\n"},
	/* With no ';' after it the technique is dropped and its name is left
	 * allocated (characterized, not endorsed). */
	{"technique T pass { vertex_shader = VS(v); } }\n"
	 "uniform float c",
	 GUARD "error 3:16 0 Expected ';'\n"
	       "leaked 1\n"},
	/* A missing or bad name skips to the next ';' past the braces, so the
	 * param goes too (characterized, not endorsed). */
	{"technique { pass { vertex_shader = VS(v); } }\n"
	 "uniform float c;",
	 GUARD "error 2:11 0 Expected 'name'\n"
	       "leaked 0\n"},
	{"technique 1 { pass { vertex_shader = VS(v); } }\n"
	 "uniform float c;",
	 GUARD "error 2:11 0 Expected 'name'\n"
	       "leaked 0\n"},
	{"technique T { pass { vertex_shader = VS(v); } }; uniform float c;",
	 "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\n"
	 "param float c\n" GUARD "leaked 0\n"},
	{"technique T { pass { vertex_shader = VS(v); } } uniform float c;",
	 "technique T { pass (null) { vs [VS ( v ) ; <end>] ps [] } }\n"
	 "param float c\n" GUARD "leaked 0\n"},
	/* A stray '}' skips to the next ';' past the technique's braces, so the
	 * technique is lost (characterized, not endorsed). */
	{"}\n"
	 "technique T { pass { vertex_shader = VS(v); } }",
	 GUARD "error 2:1 0 Expected 'type'\n"
	       "error 3:48 0 Unexpected EOF\n"
	       "leaked 0\n"},
};

static void technique_errors(void **state)
{
	(void)state;
	CHECK_CASES(technique_error_cases);
}

static const struct pass_case eof_cases[] = {
	{"technique", GUARD "error 2:10 0 Unexpected EOF\n"
			    "leaked 0\n"},
	/* End of text after the name leaves the name allocated (characterized,
	 * not endorsed). */
	{"technique T", GUARD "error 2:12 0 Unexpected EOF\n"
			      "leaked 1\n"},
	/* As does a technique without '{' that runs to the end of the text
	 * (characterized, not endorsed). */
	{"technique T pass", GUARD "error 2:17 0 Expected ';'\n"
				   "leaked 1\n"},
	{"technique T {", GUARD "error 2:14 0 Unexpected EOF\n"
				"leaked 0\n"},
	{"technique T { pass", GUARD "error 2:19 0 Unexpected EOF\n"
				     "leaked 0\n"},
	{"technique T { pass P", GUARD "error 2:21 0 Unexpected EOF\n"
				       "leaked 0\n"},
	{"technique T { pass {", GUARD "error 2:20 0 Unexpected EOF\n"
				       "leaked 0\n"},
	{"technique T { pass P {", GUARD "error 2:22 0 Unexpected EOF\n"
					 "leaked 0\n"},
	/* End of text after a command's name is reported twice: once by the
	 * '=' check, then by the pass loop (characterized, not endorsed). */
	{"technique T { pass { vertex_shader", GUARD "error 2:35 0 Unexpected EOF\n"
						     "error 2:35 0 Unexpected EOF\n"
						     "leaked 0\n"},
	{"technique T { pass { vertex_shader =", GUARD "error 2:37 0 Unexpected EOF\n"
						       "leaked 0\n"},
	{"technique T { pass { vertex_shader = compile", GUARD "error 2:38 1 compile keyword not necessary\n"
							       "error 2:45 0 Unexpected EOF\n"
							       "leaked 0\n"},
	{"technique T { pass { vertex_shader = VS(v)", GUARD "error 2:43 0 Unexpected EOF\n"
							     "leaked 0\n"},
	{"technique T { pass { vertex_shader = VS(v);", GUARD "error 2:43 0 Unexpected EOF\n"
							      "leaked 0\n"},
	{"technique T { pass { vertex_shader = VS(v); }", GUARD "error 2:46 0 Unexpected EOF\n"
								"leaked 0\n"},
	{"technique T { pass { geometry_shader", GUARD "error 2:22 0 Syntax error\n"
						       "error 2:37 0 Unexpected EOF\n"
						       "leaked 0\n"},
	{"technique T { pass { geometry_shader = GS(v)", GUARD "error 2:22 0 Syntax error\n"
							       "error 2:45 0 Unexpected EOF\n"
							       "leaked 0\n"},
	/* End of text in the skip after an entry other than a pass adds no
	 * error (characterized, not endorsed). */
	{"technique T { foo", GUARD "leaked 0\n"},
	{"technique T { foo {", GUARD "leaked 0\n"},
};

static void eof(void **state)
{
	(void)state;
	CHECK_CASES(eof_cases);
}

/* ------------------------------------------------------------------------ */

static const struct pass_case mixed_cases[] = {
	{"struct VertInOut { float4 pos : POSITION; };\n"
	 "uniform float4x4 ViewProj;\n"
	 "VertInOut VSDefault(VertInOut vert_in) { return vert_in; }\n"
	 "technique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSDraw(vert_in); } }\n"
	 "technique Other { pass { vertex_shader = VSDefault(vert_in); } }",
	 "technique Draw { pass (null) { vs [VSDefault ( vert_in ) ; <end>] ps [PSDraw ( vert_in ) ; <end>] } }\n"
	 "technique Other { pass (null) { vs [VSDefault ( vert_in ) ; <end>] ps [] } }\n"
	 "param float4x4 ViewProj\n"
	 "func VertInOut VSDefault\n" GUARD "leaked 0\n"},
	/* A missing ';' before a pass's '}' takes the next technique with it
	 * (characterized, not endorsed). */
	{"technique T { pass { vertex_shader = VS(v) } }\n"
	 "technique U { pass { pixel_shader = PS(v); } }",
	 GUARD "error 2:44 0 Expected ';'\n"
	       "leaked 0\n"},
	/* A technique without '{' skips past the next technique's braces to the
	 * end of the text: both go, and the first one's name is left allocated
	 * (characterized, not endorsed). */
	{"technique T\n"
	 "technique U { pass { pixel_shader = PS(v); } }",
	 GUARD "error 3:47 0 Expected ';'\n"
	       "leaked 1\n"},
};

static void mixed(void **state)
{
	(void)state;
	CHECK_CASES(mixed_cases);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(techniques), cmocka_unit_test(pass_errors), cmocka_unit_test(technique_errors),
		cmocka_unit_test(eof),        cmocka_unit_test(mixed),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
