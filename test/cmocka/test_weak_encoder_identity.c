/* Actual-library characterization of weak encoder pointer identity. */
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
	struct obs_encoder encoders[2] = {0};
	struct obs_weak_encoder weak = {0}, before;
	assert_false(obs_weak_encoder_references_encoder(NULL, NULL));
	assert_false(obs_weak_encoder_references_encoder(NULL, &encoders[0]));
	for (unsigned stored = 0; stored < 3; stored++) {
		weak.encoder = stored ? &encoders[stored - 1] : NULL;
		memcpy(&before, &weak, sizeof(weak));
		assert_false(obs_weak_encoder_references_encoder(&weak, NULL));
		for (unsigned target = 0; target < 2; target++) {
			assert_int_equal(obs_weak_encoder_references_encoder(&weak, &encoders[target]),
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
