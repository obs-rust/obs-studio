#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <util/bmem.h>
#include <util/cf-lexer.h>
#include <util/platform.h>

/* Characterization of the cf preprocessor in libobs/util/cf-lexer.c.h before
 * its port (#1499): #include, macros with parameters, #undef, #ifndef,
 * #if/#elif, nested blocks and define expansion. The expected values are what the C
 * code produces today, quirks included. test_cf_lexer.c has the #define,
 * #ifdef/#else, error and add/remove cases. */

#define DIR "cf_pp_test"
#define MAIN DIR "/main.c"

static void join_tokens(struct cf_token *tok, char *out, size_t out_size, size_t *count)
{
	size_t used = 0;

	*count = 0;
	out[0] = 0;
	for (; tok->type != CFTOKEN_NONE; tok++) {
		assert_true(used + tok->str.len < out_size);
		memcpy(out + used, tok->str.array, tok->str.len);
		used += tok->str.len;
		out[used] = 0;
		(*count)++;
	}
}

/* Runs the preprocessor on src and checks the joined output, the token
 * count, the define and dependency counts, the error count and the error
 * text (NULL: no errors). cf_preprocess returns true in every case here,
 * errors or not. */
static void check_pp(const char *src, const char *file, const char *sys_dir, const char *out, size_t ntok,
		     size_t defines, size_t deps, size_t errors, const char *msg)
{
	long allocs = bnum_allocs();
	struct cf_lexer lex;
	struct cf_preprocessor pp;
	struct error_data ed;
	char buf[256];
	size_t count;

	cf_lexer_init(&lex);
	cf_preprocessor_init(&pp);
	error_data_init(&ed);
	if (sys_dir)
		cf_preprocessor_add_sys_include_dir(&pp, sys_dir);

	assert_true(cf_lexer_lex(&lex, src, file));
	assert_true(cf_preprocess(&pp, &lex, &ed));

	join_tokens(cf_preprocessor_get_tokens(&pp), buf, sizeof(buf), &count);
	assert_string_equal(buf, out);
	assert_int_equal(count, ntok);
	assert_int_equal(pp.defines.num, defines);
	assert_int_equal(pp.dependencies.num, deps);

	if (msg) {
		char *text = error_data_buildstring(&ed);
		assert_int_equal(ed.errors.num, errors);
		assert_string_equal(text, msg);
		bfree(text);
	} else {
		assert_int_equal(ed.errors.num, 0);
	}

	error_data_free(&ed);
	cf_preprocessor_free(&pp);
	cf_lexer_free(&lex);
	assert_int_equal(bnum_allocs(), allocs);
}

static void write_file(const char *path, const char *text)
{
	assert_true(os_quick_write_utf8_file(path, text, strlen(text), false));
}

static int setup_files(void **state)
{
	UNUSED_PARAMETER(state);

	os_mkdir(DIR);
	os_mkdir(DIR "/sys");
	write_file(DIR "/a.inc", "#define A_VAL 1\nfrom_a\n");
	write_file(DIR "/nest.inc", "#include \"b.inc\"\n");
	write_file(DIR "/b.inc", "#define B_VAL 2\nfrom_b\n");
	write_file(DIR "/sys/s.inc", "#define S_VAL 9\nfrom_s\n");
	return 0;
}

static int teardown_files(void **state)
{
	UNUSED_PARAMETER(state);

	os_unlink(DIR "/a.inc");
	os_unlink(DIR "/nest.inc");
	os_unlink(DIR "/b.inc");
	os_unlink(DIR "/sys/s.inc");
	os_rmdir(DIR "/sys");
	os_rmdir(DIR);
	return 0;
}

static void include_test(void **state)
{
	UNUSED_PARAMETER(state);

	/* The file is read relative to the including file. Its directive line
	 * stays as a newline, and its defines are kept. */
	check_pp("#include \"a.inc\"\nA_VAL x", MAIN, NULL, "\nfrom_a\n\n 1 x", 8, 1, 1, 0, NULL);

	/* An include inside an include: both files are dependencies. */
	check_pp("#include \"nest.inc\"\nB_VAL", MAIN, NULL, "\nfrom_b\n\n\n 2", 7, 1, 2, 0, NULL);

	/* Twice: the text comes twice, the define is added twice, the file is
	 * one dependency. */
	check_pp("#include \"a.inc\"\n#include \"a.inc\"\nA_VAL", MAIN, NULL, "\nfrom_a\n\n\nfrom_a\n\n 1", 10, 2, 1, 0,
		 NULL);
}

static void include_errors_test(void **state)
{
	UNUSED_PARAMETER(state);

	/* The error text runs from the path token to the end of the source,
	 * not only the path. */
	check_pp("#include \"nope.inc\"\nx", MAIN, NULL, "\nx", 2, 0, 0, 1,
		 MAIN " (1, 10): Could not open file '\"nope.inc\"\nx'\n");

	check_pp("#include x\ny", MAIN, NULL, "\ny", 2, 0, 0, 1, MAIN " (1, 10): Expected string\n");
}

static void include_sys_test(void **state)
{
	UNUSED_PARAMETER(state);

	/* <...> includes are skipped without an error, with or without a
	 * system include directory. */
	check_pp("#include <s.inc>\nS_VAL", MAIN, DIR "/sys", "\nS_VAL", 2, 0, 0, 0, NULL);
	check_pp("#include <s.inc>\nS_VAL", MAIN, NULL, "\nS_VAL", 2, 0, 0, 0, NULL);
}

static void macro_params_test(void **state)
{
	UNUSED_PARAMETER(state);

	check_pp("#define ADD(a, b) a + b\nADD(1, 2);", "t.c", NULL, "\n  1  +   2 ;", 13, 1, 0, 0, NULL);

	/* An argument that is itself a call is expanded. */
	check_pp("#define ADD(a, b) (a+b)\nADD(ADD(1,2), 3)", "t.c", NULL, "\n (  ( 1 + 2 ) +  3 )", 21, 1, 0, 0, NULL);
}

static void macro_params_errors_test(void **state)
{
	UNUSED_PARAMETER(state);

	/* Too few: the missing parameter stays as its name. */
	check_pp("#define ADD(a, b) a + b\nADD(1)", "t.c", NULL, "\n  1  + b", 9, 1, 0, 1,
		 "t.c (2, 6): Mismatching number of macro parameters\n");

	/* Too many: the extra argument is dropped. */
	check_pp("#define ADD(a, b) a + b\nADD(1, 2, 3)", "t.c", NULL, "\n  1  +   2 ", 12, 1, 0, 1,
		 "t.c (2, 12): Mismatching number of macro parameters\n");

	/* No call: the body is inserted with the parameter names. */
	check_pp("#define ADD(a, b) a + b\nADD;", "t.c", NULL, "\n a + b", 7, 1, 0, 1, "t.c (2, 4): Expected '('\n");

	/* An unterminated parameter list: no define, the call stays. */
	check_pp("#define ADD(a, b a + b\nADD(1,2)", "t.c", NULL, "\nADD(1,2)", 7, 0, 0, 1,
		 "t.c (1, 18): Expected ',' or ')'\n");
}

static void undef_test(void **state)
{
	UNUSED_PARAMETER(state);

	check_pp("#define FOO 1\n#undef FOO\nFOO", "t.c", NULL, "\n\nFOO", 3, 0, 0, 0, NULL);
	check_pp("#undef NOPE\nx", "t.c", NULL, "\nx", 2, 0, 0, 0, NULL);

	/* A second #define adds a second entry; the first one is used. */
	check_pp("#define FOO 1\n#define FOO 2\nFOO", "t.c", NULL, "\n\n 1", 4, 2, 0, 0, NULL);
}

static void ifndef_nested_test(void **state)
{
	UNUSED_PARAMETER(state);

	check_pp("#ifndef FOO\nyes\n#else\nno\n#endif", "t.c", NULL, "\nyes\n", 3, 0, 0, 0, NULL);
	check_pp("#define FOO\n#ifndef FOO\nyes\n#else\nno\n#endif", "t.c", NULL, "\n\nno\n", 4, 1, 0, 0, NULL);

	/* A block inside a skipped block is skipped as a whole. */
	check_pp("#ifdef A\n#ifdef B\nab\n#else\nanb\n#endif\n#else\nna\n#endif", "t.c", NULL, "\nna\n", 3, 0, 0, 0,
		 NULL);

	/* A #define in a skipped block is not added. */
	check_pp("#ifdef A\n#define X 1\n#endif\nX", "t.c", NULL, "\nX", 2, 0, 0, 0, NULL);
}

static void expansion_test(void **state)
{
	UNUSED_PARAMETER(state);

	/* A define whose value is another define is expanded again. */
	check_pp("#define A B\n#define B 3\nA", "t.c", NULL, "\n\n  3", 5, 2, 0, 0, NULL);

	/* A backslash continues the value on the next line. */
	check_pp("#define M 1 \\\n + 2\nM", "t.c", NULL, "\n 1  + 2", 7, 1, 0, 0, NULL);

	/* A '#' that is not first on its line is plain text. */
	check_pp("x # define Y 1\nY", "t.c", NULL, "x # define Y 1\nY", 11, 0, 0, 0, NULL);
}

static void other_directives_test(void **state)
{
	UNUSED_PARAMETER(state);

	check_pp("#pragma once\nx", "t.c", NULL, " \nx", 3, 0, 0, 1,
		 "t.c (1, 9): Unexpected token after preprocessor, expected newline\n");

	/* A second #else is accepted; the #endif after it then has no block. */
	check_pp("#ifdef A\n#else\n#else\n#endif", "t.c", NULL, "\n\n", 2, 0, 0, 1,
		 "t.c (4, 2): #endif outside of #if/#ifdef/#ifndef block\n");

	check_pp("#define\nx", "t.c", NULL, "\nx", 2, 0, 0, 1, "t.c (1, 8): Expected identifier\n");
	check_pp("#define 3 4\nx", "t.c", NULL, "\nx", 2, 0, 0, 1, "t.c (1, 9): Expected identifier\n");
}

static void if_test(void **state)
{
	UNUSED_PARAMETER(state);

	check_pp("#if 1\nx\n#endif", "t.c", NULL, "\nx\n", 3, 0, 0, 0, NULL);
	check_pp("#if 0\nx\n#else\ny\n#endif", "t.c", NULL, "\ny\n", 3, 0, 0, 0, NULL);

	/* A define with a number value, also through another define. */
	check_pp("#define V 3\n#if V\nx\n#endif", "t.c", NULL, "\n\nx\n", 4, 1, 0, 0, NULL);
	check_pp("#define A B\n#define B 5\n#if A == 5\nx\n#endif", "t.c", NULL, "\n\n\nx\n", 5, 2, 0, 0, NULL);

	/* The comparators, and a number as sscanf "%i" reads it. */
	check_pp("#define V 3\n#if V == 3\nx\n#else\ny\n#endif", "t.c", NULL, "\n\nx\n", 4, 1, 0, 0, NULL);
	check_pp("#define V 3\n#if V != 3\nx\n#else\ny\n#endif", "t.c", NULL, "\n\ny\n", 4, 1, 0, 0, NULL);
	check_pp("#if 2 >= 3\na\n#endif\n#if 2 <= 3\nb\n#endif", "t.c", NULL, "\n\nb\n", 4, 0, 0, 0, NULL);
	check_pp("#if 2 < 3\na\n#endif\n#if 2 > 3\nb\n#endif", "t.c", NULL, "\na\n\n", 4, 0, 0, 0, NULL);
	check_pp("#if 0x10 == 16\nx\n#endif", "t.c", NULL, "\nx\n", 3, 0, 0, 0, NULL);

	/* An #if in a skipped block is skipped with it. */
	check_pp("#ifdef X\n#if 1\na\n#endif\n#endif\nz", "t.c", NULL, "\nz", 2, 0, 0, 0, NULL);
}

static void if_errors_test(void **state)
{
	UNUSED_PARAMETER(state);

	/* On an error the line is dropped and the block is kept, so the #endif
	 * then has no block. A negative number, an undefined name, a define
	 * with text and a macro with parameters are not values. */
	check_pp("#if NOPE\nx\n#endif", "t.c", NULL, "\nx\n", 3, 0, 0, 2,
		 "t.c (1, 5): Expected identifier or number\n"
		 "t.c (3, 2): #endif outside of #if/#ifdef/#ifndef block\n");
	check_pp("#if -1\nx\n#endif", "t.c", NULL, "\nx\n", 3, 0, 0, 2,
		 "t.c (1, 5): Expected identifier or number\n"
		 "t.c (3, 2): #endif outside of #if/#ifdef/#ifndef block\n");
	check_pp("#define V abc\n#if V\nx\n#endif", "t.c", NULL, "\n\nx\n", 4, 1, 0, 2,
		 "t.c (2, 5): Expected identifier or number\n"
		 "t.c (4, 2): #endif outside of #if/#ifdef/#ifndef block\n");
	check_pp("#define M(a) a\n#if M\nx\n#endif", "t.c", NULL, "\n\nx\n", 4, 1, 0, 2,
		 "t.c (2, 5): Expected identifier or number\n"
		 "t.c (4, 2): #endif outside of #if/#ifdef/#ifndef block\n");
	check_pp("#if 1 =! 2\nx\n#endif", "t.c", NULL, "\nx\n", 3, 0, 0, 2,
		 "t.c (1, 10): Expected comparator\n"
		 "t.c (3, 2): #endif outside of #if/#ifdef/#ifndef block\n");
	check_pp("#if 1 2\nx\n#endif", "t.c", NULL, "\nx\n", 3, 0, 0, 2,
		 "t.c (1, 7): Expected comparator\n"
		 "t.c (3, 2): #endif outside of #if/#ifdef/#ifndef block\n");
	check_pp("#if 1 ==\nx\n#endif", "t.c", NULL, "\nx\n", 3, 0, 0, 2,
		 "t.c (1, 9): Expected identifier or number\n"
		 "t.c (3, 2): #endif outside of #if/#ifdef/#ifndef block\n");

	check_pp("#if 1\nx", "t.c", NULL, "\nx", 2, 0, 0, 1, "t.c (2, 2): Unexpected end of file before #endif\n");
}

static void elif_test(void **state)
{
	UNUSED_PARAMETER(state);

	check_pp("#if 0\na\n#elif 1\nb\n#else\nc\n#endif", "t.c", NULL, "\nb\n", 3, 0, 0, 0, NULL);
	check_pp("#if 0\na\n#elif 0\nb\n#else\nc\n#endif", "t.c", NULL, "\nc\n", 3, 0, 0, 0, NULL);
	check_pp("#ifdef X\na\n#elif 1\nb\n#endif", "t.c", NULL, "\nb\n", 3, 0, 0, 0, NULL);

	/* After a true branch the #elif condition is read as text on a
	 * directive line. */
	check_pp("#if 1\na\n#elif 1\nb\n#endif", "t.c", NULL, "\na\n", 3, 0, 0, 1,
		 "t.c (3, 7): Unexpected token after preprocessor, expected newline\n");

	check_pp("#elif 1\nx", "t.c", NULL, " \nx", 3, 0, 0, 2,
		 "t.c (1, 2): #elif outside of #if/#ifdef/#ifndef block\n"
		 "t.c (1, 7): Unexpected token after preprocessor, expected newline\n");
	check_pp("#else\nx", "t.c", NULL, "\nx", 2, 0, 0, 1, "t.c (1, 2): #else outside of #if/#ifdef/#ifndef block\n");
}

static void empty_source_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct cf_lexer lex;

	/* The lexer refuses empty text, so there is nothing to preprocess. */
	cf_lexer_init(&lex);
	assert_false(cf_lexer_lex(&lex, "", "t.c"));
	cf_lexer_free(&lex);
	assert_int_equal(bnum_allocs(), allocs);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(include_test),
		cmocka_unit_test(include_errors_test),
		cmocka_unit_test(include_sys_test),
		cmocka_unit_test(macro_params_test),
		cmocka_unit_test(macro_params_errors_test),
		cmocka_unit_test(undef_test),
		cmocka_unit_test(ifndef_nested_test),
		cmocka_unit_test(expansion_test),
		cmocka_unit_test(if_test),
		cmocka_unit_test(if_errors_test),
		cmocka_unit_test(elif_test),
		cmocka_unit_test(other_directives_test),
		cmocka_unit_test(empty_source_test),
	};

	return cmocka_run_group_tests(tests, setup_files, teardown_files);
}
