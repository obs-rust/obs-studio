/* Public-API characterization of obs_enum_modules before extraction/porting.
 * The list is newest-opened first, excludes disabled modules and calls a void
 * callback with borrowed module pointers and the original parameter. There is
 * no early stop, lock or lifetime pin: the C loop reads module->next AFTER the
 * callback. Nested enumeration and prepending via obs_open_module are safe
 * while the existing nodes remain alive. Destroying the current node/core is
 * outside this test's valid domain. NULL callback on a nonempty list would call
 * a NULL function pointer (characterized, not endorsed); no-core/empty is safe.
 * This Linux fixture reuses the existing public-API loader's stub module. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

struct visits {
	obs_module_t *modules[8];
	size_t count;
};

static void collect(void *param, obs_module_t *module)
{
	struct visits *visits = param;
	assert_true(visits->count < 8);
	visits->modules[visits->count++] = module;
}

static size_t null_param_calls;

static void accept_null_param(void *param, obs_module_t *module)
{
	assert_null(param);
	assert_non_null(module);
	null_param_calls++;
}

static obs_module_t *open_module(void)
{
	obs_module_t *module = NULL;
	assert_int_equal(obs_open_module(&module, CORE_STUB_PATH, ""), MODULE_SUCCESS);
	assert_non_null(module);
	return module;
}

static void expect_order(const struct visits *visits, obs_module_t *a, obs_module_t *b, obs_module_t *c)
{
	assert_int_equal(visits->count, 3);
	assert_ptr_equal(visits->modules[0], a);
	assert_ptr_equal(visits->modules[1], b);
	assert_ptr_equal(visits->modules[2], c);
}

static void absent_core(void **state)
{
	(void)state;
	assert_false(obs_initialized());
	struct visits visits = {0};
	obs_enum_modules(collect, &visits);
	obs_enum_modules(NULL, NULL);
	assert_int_equal(visits.count, 0);
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void empty_and_disabled(void **state)
{
	(void)state;
	struct visits visits = {0};
	obs_enum_modules(NULL, NULL);
	obs_enum_modules(collect, &visits);
	assert_int_equal(visits.count, 0);
	obs_module_t *disabled = NULL;
	assert_true(obs_create_disabled_module(&disabled, CORE_STUB_PATH, "", OBS_MODULE_DISABLED));
	obs_enum_modules(NULL, NULL);
	obs_enum_modules(collect, &visits);
	assert_int_equal(visits.count, 0);
}

static void reverse_open_order(void **state)
{
	(void)state;
	obs_module_t *a = open_module();
	obs_module_t *b = open_module();
	obs_module_t *c = open_module();
	assert_ptr_not_equal(a, b);
	assert_ptr_not_equal(b, c);
	struct visits visits = {0};
	obs_enum_modules(collect, &visits);
	expect_order(&visits, c, b, a);
	null_param_calls = 0;
	obs_enum_modules(accept_null_param, NULL);
	assert_int_equal(null_param_calls, 3);
}

struct reentrant {
	struct visits outer;
	struct visits nested;
	obs_module_t *added;
};

static void recurse_and_prepend(void *param, obs_module_t *module)
{
	struct reentrant *calls = param;
	collect(&calls->outer, module);
	if (calls->outer.count == 1) {
		calls->added = open_module();
		obs_enum_modules(collect, &calls->nested);
	}
}

static void reentrant_enumeration(void **state)
{
	(void)state;
	obs_module_t *a = open_module();
	obs_module_t *b = open_module();
	struct reentrant calls = {0};
	obs_enum_modules(recurse_and_prepend, &calls);
	assert_int_equal(calls.outer.count, 2);
	assert_ptr_equal(calls.outer.modules[0], b);
	assert_ptr_equal(calls.outer.modules[1], a);
	expect_order(&calls.nested, calls.added, b, a);
	struct visits after = {0};
	obs_enum_modules(collect, &after);
	expect_order(&after, calls.added, b, a);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(absent_core),
		cmocka_unit_test_setup_teardown(empty_and_disabled, setup, teardown),
		cmocka_unit_test_setup_teardown(reverse_open_order, setup, teardown),
		cmocka_unit_test_setup_teardown(reentrant_enumeration, setup, teardown),
		cmocka_unit_test(absent_core),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
