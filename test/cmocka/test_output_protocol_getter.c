/* Characterize borrowed protocols through the actual libobs export. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void borrowed_protocols(void **state)
{
	(void)state;
	struct obs_output output = {0};
	struct obs_output before;
	const char bytes[] = {'r', 't', 'm', 'p', ';', (char)0xff, 0};
	const char *values[] = {NULL, "", "rtmp;rtmps", bytes};
	const uint32_t flags[] = {0, OBS_OUTPUT_SERVICE, OBS_OUTPUT_VIDEO, UINT32_MAX,
				  UINT32_MAX & ~(uint32_t)OBS_OUTPUT_SERVICE};
	assert_null(obs_output_get_protocols(NULL));
	for (size_t v = 0; v < 4; v++) {
		for (size_t f = 0; f < 5; f++) {
			output.info.protocols = values[v];
			output.info.flags = flags[f];
			memcpy(&before, &output, sizeof(output));
			const char *expected = (flags[f] & OBS_OUTPUT_SERVICE) ? values[v] : NULL;
			assert_ptr_equal(obs_output_get_protocols(&output), expected);
			assert_ptr_equal(obs_output_get_protocols(&output), expected);
			assert_memory_equal(&output, &before, sizeof(output));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_protocols)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
