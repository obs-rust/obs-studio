/* Actual-library characterization of weak object pointer identity. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void identity(void **state)
{
	(void)state;
	struct obs_context_data objects[2] = {0};
	struct obs_weak_object weak = {0}, before;
	assert_false(obs_weak_object_references_object(NULL, NULL));
	assert_false(obs_weak_object_references_object(NULL, &objects[0]));
	for (unsigned stored = 0; stored < 3; stored++) {
		weak.object = stored ? &objects[stored - 1] : NULL;
		memcpy(&before, &weak, sizeof(weak));
		assert_false(obs_weak_object_references_object(&weak, NULL));
		for (unsigned target = 0; target < 2; target++) {
			assert_int_equal(obs_weak_object_references_object(&weak, &objects[target]),
					 stored == target + 1);
			assert_memory_equal(&before, &weak, sizeof(weak));
		}
	}
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(identity)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
