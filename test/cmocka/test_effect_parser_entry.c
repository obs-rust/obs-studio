#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/effect-parser.h>
#include <graphics/effect-parser-compile.h>
#include <graphics/effect.h>
#include <util/bmem.h>

/* Characterize ep_parse itself with the real C lexer and child parsers.
 * Only the graphics backend name and final compile step are recording stand-ins.
 * No device, display or shader is needed. ep_compile's component tests separately
 * pin its effects. This entry test must keep calling the C implementation until
 * the standalone entry unit is swapped in a follow-up PR. */
static const char *backend_name;
static bool compile_result;
static unsigned compile_calls;
static struct effect_parser *compiled_parser;

const char *gs_preprocessor_name(void);
const char *gs_preprocessor_name(void)
{
	return backend_name;
}

bool ep_compile(struct effect_parser *ep)
{
	compile_calls++;
	compiled_parser = ep;
	return compile_result;
}

static void check_parse(const char *text, const char *backend, bool compiler_ok, bool expected_ok,
			unsigned expected_calls, size_t params, size_t structs, size_t samplers, size_t funcs,
			size_t techniques, size_t errors, const char *param_name, const char *first_error)
{
	const long before = bnum_allocs();
	struct effect_parser ep;
	struct gs_effect effect = {0};
	backend_name = backend;
	compile_result = compiler_ok;
	compile_calls = 0;
	compiled_parser = NULL;
	ep_init(&ep);

	const bool ok = ep_parse(&ep, &effect, text, "entry.effect");
	assert_int_equal(ok, expected_ok);
	assert_ptr_equal(ep.effect, &effect);
	assert_int_equal(compile_calls, expected_calls);
	assert_ptr_equal(compiled_parser, expected_calls ? &ep : NULL);
	assert_int_equal(ep.params.num, params);
	assert_int_equal(ep.structs.num, structs);
	assert_int_equal(ep.samplers.num, samplers);
	assert_int_equal(ep.funcs.num, funcs);
	assert_int_equal(ep.techniques.num, techniques);
	assert_int_equal(ep.cfp.error_list.errors.num, errors);
	if (param_name)
		assert_string_equal(ep.params.array[0].name, param_name);
	if (first_error)
		assert_string_equal(ep.cfp.error_list.errors.array[0].error, first_error);

	ep_free(&ep);
	assert_int_equal(bnum_allocs(), before);
}

static void empty_and_compile_result(void **state)
{
	(void)state;
	/* Characterized, not endorsed: an empty source fails before compile. */
	check_parse("", NULL, true, false, 0, 0, 0, 0, 0, 0, 0, NULL, NULL);
	check_parse(" \t\r\n;;; // comment\n", NULL, true, true, 1, 0, 0, 0, 0, 0, 0, NULL, NULL);
	check_parse("float value;", NULL, false, false, 1, 1, 0, 0, 0, 0, 0, "value", NULL);
}

static void backend_preprocessor_definition(void **state)
{
	(void)state;
	const char *text = "#ifdef ENTRY_BACKEND\nfloat selected;\n#else\nfloat fallback;\n#endif\n";
	check_parse(text, "ENTRY_BACKEND", true, true, 1, 1, 0, 0, 0, 0, 0, "selected", NULL);
	check_parse(text, NULL, true, true, 1, 1, 0, 0, 0, 0, 0, "fallback", NULL);
	check_parse(text, "OTHER_BACKEND", true, true, 1, 1, 0, 0, 0, 0, 0, "fallback", NULL);
}

static void dispatch_all_top_level_kinds(void **state)
{
	(void)state;
	check_parse("; struct S { float x; }; sampler_state sample { Filter = Linear; }; "
		    "float value; float fun() { return value; } technique Draw { pass {} } ;",
		    NULL, true, true, 1, 1, 1, 1, 1, 1, 0, "value", NULL);
}

static void parse_errors_gate_compile_but_warnings_do_not(void **state)
{
	(void)state;
	check_parse("uniform uniform float value;", NULL, true, true, 1, 1, 0, 0, 0, 0, 1, "value",
		    "'uniform' keyword already specified");
	check_parse("uniform float fun() {}", NULL, true, false, 0, 0, 0, 0, 1, 0, 1, NULL,
		    "'uniform' keyword cannot be used with a function");
	/* Preprocessor errors gate compilation but do not stop declaration dispatch. */
	check_parse("#error stop_here\nfloat ignored;", NULL, true, false, 0, 1, 0, 0, 0, 0, 1, "ignored", NULL);
}

static void unexpected_braces_recover_to_next_declaration(void **state)
{
	(void)state;
	/* Characterized, not endorsed: parsing keeps declarations after the error,
	 * even though the accumulated error prevents the compile step. */
	check_parse("{{}} ; float recovered;", NULL, true, false, 0, 1, 0, 0, 0, 0, 1, "recovered",
		    "Unexpected code segment");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(empty_and_compile_result),
		cmocka_unit_test(backend_preprocessor_definition),
		cmocka_unit_test(dispatch_all_top_level_kinds),
		cmocka_unit_test(parse_errors_gate_compile_but_warnings_do_not),
		cmocka_unit_test(unexpected_braces_recover_to_next_declaration),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
