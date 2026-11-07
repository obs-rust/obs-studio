#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include <obs.h>
#include <util/bmem.h>

/* Characterized, not endorsed: count is not reset, and only the table is
 * owned. Its entries may point to borrowed storage; cleanup must not read
 * count entries or free the strings. No OBS startup or display is needed. */
static void cleanup_table_only(void **state)
{
	(void)state;
	const size_t counts[] = {0, 1, 3, SIZE_MAX};
	for (size_t i = 0; i < sizeof(counts) / sizeof(counts[0]); i++) {
		long baseline = bnum_allocs();
		char *borrowed = bstrdup("module-name");
		char **table = bmalloc(sizeof(*table));
		table[0] = borrowed;
		struct obs_module_failure_info info = {table, counts[i]};
		assert_int_equal(bnum_allocs(), baseline + 2);
		obs_module_failure_info_free(&info);
		assert_null(info.failed_modules);
		assert_true(info.count == counts[i]);
		assert_int_equal(bnum_allocs(), baseline + 1);
		assert_string_equal(borrowed, "module-name");
		obs_module_failure_info_free(&info);
		assert_null(info.failed_modules);
		assert_true(info.count == counts[i]);
		assert_int_equal(bnum_allocs(), baseline + 1);
		bfree(borrowed);
		assert_int_equal(bnum_allocs(), baseline);
	}
}

static void cleanup_empty(void **state)
{
	(void)state;
	struct obs_module_failure_info info = {NULL, SIZE_MAX};
	long baseline = bnum_allocs();
	obs_module_failure_info_free(&info);
	assert_null(info.failed_modules);
	assert_true(info.count == SIZE_MAX);
	assert_int_equal(bnum_allocs(), baseline);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(cleanup_table_only), cmocka_unit_test(cleanup_empty)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
