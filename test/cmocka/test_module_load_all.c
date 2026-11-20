/* Deprecated module-loading exports are intentional no-ops, characterized
 * without startup. This baseline must not be changed by the later Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

/* Match test_no_core_getters.c: deprecated exports remain ABI contracts. */
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

#define obs_load_all_modules baseline_load_all_modules
#define obs_load_all_modules2 baseline_load_all_modules2
#include "libobs/obs-module-load-all.c.h"
#undef obs_load_all_modules2
#undef obs_load_all_modules

static unsigned log_calls;
#if defined(__GNUC__) || defined(__clang__)
static void record_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void record_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	log_calls++;
}

static void characterize(void (*load_all)(void), void (*load_all2)(struct obs_module_failure_info *))
{
	char first[] = "failed-module-sentinel";
	char second[] = "another-sentinel";
	char *failed[] = {first, second, NULL};
	char *failed_before[3];
	memcpy(failed_before, failed, sizeof(failed));
	struct obs_module_failure_info info;
	memset(&info, 0xa5, sizeof(info));
	info.failed_modules = failed;
	/* Deliberately not the array length: even the count is caller-owned. */
	info.count = SIZE_MAX - 17;
	unsigned char before[sizeof(info)];
	memcpy(before, &info, sizeof(info));
	long allocations = bnum_allocs();
	assert_false(obs_initialized());
	/* With no core, a loader cannot enumerate or publish any modules. A
	 * reactivated loader would access the missing core and fail this test. */
	for (unsigned repeat = 0; repeat < 3; repeat++) {
		load_all();
		assert_false(obs_initialized());
		assert_int_equal(bnum_allocs(), allocations);
		assert_int_equal(log_calls, 0);

		load_all2(&info);
		assert_memory_equal(&info, before, sizeof(info));
		assert_ptr_equal(info.failed_modules, failed);
		assert_true(info.count == SIZE_MAX - 17);
		assert_memory_equal(failed, failed_before, sizeof(failed));
		assert_memory_equal(first, "failed-module-sentinel", sizeof(first));
		assert_memory_equal(second, "another-sentinel", sizeof(second));
		assert_false(obs_initialized());
		assert_int_equal(bnum_allocs(), allocations);
		assert_int_equal(log_calls, 0);

		load_all2(NULL);
		assert_false(obs_initialized());
		assert_int_equal(bnum_allocs(), allocations);
		assert_int_equal(log_calls, 0);
	}
}

static void exported_noops(void **state)
{
	(void)state;
	characterize(obs_load_all_modules, obs_load_all_modules2);
}

static void c_baseline_noops(void **state)
{
	(void)state;
	characterize(baseline_load_all_modules, baseline_load_all_modules2);
}

int main(void)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(record_log, NULL);
	const struct CMUnitTest tests[] = {cmocka_unit_test(exported_noops), cmocka_unit_test(c_baseline_noops)};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	base_set_log_handler(old_handler, old_param);
	return result;
}
