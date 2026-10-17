#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>

#include <cmocka.h>

#include "util/pipe.h"

static void test_create_uses_sh_as_argv0(void **state)
{
	(void)state;
	os_process_pipe_t *pp = os_process_pipe_create("printf '%s' \"$0\"", "r");
	assert_non_null(pp);

	char output[32] = {0};
	size_t len = 0;
	while (len < sizeof(output) - 1) {
		size_t n = os_process_pipe_read(pp, (uint8_t *)output + len, sizeof(output) - 1 - len);
		if (n == 0)
			break;
		len += n;
	}
	output[len] = '\0';
	int status = os_process_pipe_destroy(pp);

	assert_int_equal(status, 0);
	assert_string_equal(output, "sh");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_create_uses_sh_as_argv0),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
