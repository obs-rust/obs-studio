/* Actual-library characterization of the generic weak-object lifetime query. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <limits.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void null_is_expired(void **state)
{
	(void)state;
	assert_true(obs_weak_object_expired(NULL));
}

static void check_counts(const long *owners, size_t count, bool expected)
{
	const long weak_counts[] = {LONG_MIN, -2, -1, 0, 1, LONG_MAX};
	struct obs_context_data object = {0};
	for (size_t i = 0; i < count; i++) {
		for (size_t j = 0; j < sizeof(weak_counts) / sizeof(weak_counts[0]); j++) {
			for (unsigned stored = 0; stored < 2; stored++) {
				struct obs_weak_object weak = {0}, before;
				weak.ref.refs = owners[i];
				weak.ref.weak_refs = weak_counts[j];
				weak.object = stored ? &object : NULL;
				memcpy(&before, &weak, sizeof(weak));
				assert_int_equal(obs_weak_object_expired(&weak), expected);
				assert_memory_equal(&before, &weak, sizeof(weak));
			}
		}
	}
}

static void nonnegative_owners_are_alive(void **state)
{
	(void)state;
	/* Zero represents one remaining strong owner in this biased counter. */
	const long owners[] = {0, 1, LONG_MAX};
	check_counts(owners, sizeof(owners) / sizeof(owners[0]), false);
}

static void negative_owners_are_expired(void **state)
{
	(void)state;
	const long owners[] = {LONG_MIN, -2, -1};
	check_counts(owners, sizeof(owners) / sizeof(owners[0]), true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_is_expired),
					   cmocka_unit_test(nonnegative_owners_are_alive),
					   cmocka_unit_test(negative_owners_are_expired)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
