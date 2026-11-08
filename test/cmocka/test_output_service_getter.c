/* Characterize the borrowed service pointer through the actual library export. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void borrowed_service(void **state)
{
	(void)state;
	struct obs_output output = {0}, before;
	struct obs_service services[2] = {0};
	assert_null(obs_output_get_service(NULL));
	for (unsigned i = 0; i < 3; i++) {
		output.service = i ? &services[i - 1] : NULL;
		memcpy(&before, &output, sizeof(output));
		assert_ptr_equal(obs_output_get_service(&output), output.service);
		assert_ptr_equal(obs_output_get_service(&output), output.service);
		assert_memory_equal(&output, &before, sizeof(output));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_service)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
