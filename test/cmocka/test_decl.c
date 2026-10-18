#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include <util/bmem.h>
#include <callback/decl.h>

/* Characterization tests: the expected values are what
 * libobs/callback/decl.c produces today (#1438). */

static void check_param(const struct decl_info *decl, size_t idx, const char *name, enum call_param_type type,
			uint32_t flags)
{
	assert_true(idx < decl->params.num);
	assert_string_equal(decl->params.array[idx].name, name);
	assert_int_equal(decl->params.array[idx].type, type);
	assert_int_equal(decl->params.array[idx].flags, flags);
}

/* A failed parse frees and zeroes the whole decl_info, decl_string too. */
static void check_fails(const char *str)
{
	long allocs = bnum_allocs();
	struct decl_info decl = {0};

	assert_false(parse_decl_string(&decl, str));
	assert_null(decl.name);
	assert_null(decl.decl_string);
	assert_int_equal(decl.params.num, 0);
	assert_null(decl.params.array);
	assert_int_equal(bnum_allocs(), allocs);
}

static void void_function_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	const char *str = "void f(int a)";
	struct decl_info decl = {0};

	assert_true(parse_decl_string(&decl, str));
	assert_string_equal(decl.name, "f");
	assert_ptr_equal(decl.decl_string, str);
	assert_int_equal(decl.params.num, 1);
	/* parameters without a specifier are input parameters */
	check_param(&decl, 0, "a", CALL_PARAM_TYPE_INT, CALL_PARAM_IN);

	decl_info_free(&decl);
	assert_int_equal(bnum_allocs(), allocs);
}

static void return_type_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct decl_info decl = {0};

	/* A non-void return type is appended as an output parameter "return". */
	assert_true(parse_decl_string(&decl, "int get(in ptr src, out string s, in out float x)"));
	assert_string_equal(decl.name, "get");
	assert_int_equal(decl.params.num, 4);
	check_param(&decl, 0, "src", CALL_PARAM_TYPE_PTR, CALL_PARAM_IN);
	check_param(&decl, 1, "s", CALL_PARAM_TYPE_STRING, CALL_PARAM_OUT);
	check_param(&decl, 2, "x", CALL_PARAM_TYPE_FLOAT, CALL_PARAM_IN | CALL_PARAM_OUT);
	check_param(&decl, 3, "return", CALL_PARAM_TYPE_INT, CALL_PARAM_OUT);

	decl_info_free(&decl);
	assert_int_equal(bnum_allocs(), allocs);
}

static void all_types_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct decl_info decl = {0};

	assert_true(parse_decl_string(&decl, "bool f(int a, float b, bool c, ptr d, string e)"));
	assert_int_equal(decl.params.num, 6);
	check_param(&decl, 0, "a", CALL_PARAM_TYPE_INT, CALL_PARAM_IN);
	check_param(&decl, 1, "b", CALL_PARAM_TYPE_FLOAT, CALL_PARAM_IN);
	check_param(&decl, 2, "c", CALL_PARAM_TYPE_BOOL, CALL_PARAM_IN);
	check_param(&decl, 3, "d", CALL_PARAM_TYPE_PTR, CALL_PARAM_IN);
	check_param(&decl, 4, "e", CALL_PARAM_TYPE_STRING, CALL_PARAM_IN);
	check_param(&decl, 5, "return", CALL_PARAM_TYPE_BOOL, CALL_PARAM_OUT);

	decl_info_free(&decl);
	assert_int_equal(bnum_allocs(), allocs);
}

static void empty_params_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct decl_info decl = {0};

	assert_true(parse_decl_string(&decl, "void destroy()"));
	assert_string_equal(decl.name, "destroy");
	assert_int_equal(decl.params.num, 0);
	decl_info_free(&decl);

	/* a non-void function without parameters only has "return" */
	assert_true(parse_decl_string(&decl, "ptr get()"));
	assert_int_equal(decl.params.num, 1);
	check_param(&decl, 0, "return", CALL_PARAM_TYPE_PTR, CALL_PARAM_OUT);
	decl_info_free(&decl);

	assert_int_equal(bnum_allocs(), allocs);
}

static void failures_test(void **state)
{
	UNUSED_PARAMETER(state);

	/* types */
	check_fails("void f(foo a)");
	check_fails("void f(void a)");
	check_fails("foo f(int a)");
	/* names */
	check_fails("void f(int a, int a)");
	check_fails("void f(int return)");
	check_fails("void f(int int)");
	check_fails("void int(int a)");
	/* specifiers */
	check_fails("void f(in in int a)");
	check_fails("void f(out out int a)");
	/* structure */
	check_fails("void f(int a");
	check_fails("");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(void_function_test), cmocka_unit_test(return_type_test),
		cmocka_unit_test(all_types_test),     cmocka_unit_test(empty_params_test),
		cmocka_unit_test(failures_test),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
