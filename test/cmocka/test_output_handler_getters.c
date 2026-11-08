/* Characterize borrowed output handlers through the actual library exports. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void borrowed_handlers(void **state)
{
	(void)state;
	struct obs_output output = {0}, before;
	max_align_t signal_token, proc_token;
	signal_handler_t *signals = (signal_handler_t *)&signal_token;
	proc_handler_t *procs = (proc_handler_t *)&proc_token;
	assert_null(obs_output_get_signal_handler(NULL));
	assert_null(obs_output_get_proc_handler(NULL));
	for (unsigned s = 0; s < 2; s++) {
		for (unsigned p = 0; p < 2; p++) {
			output.context.signals = s ? signals : NULL;
			output.context.procs = p ? procs : NULL;
			memcpy(&before, &output, sizeof(output));
			assert_ptr_equal(obs_output_get_signal_handler(&output), output.context.signals);
			assert_ptr_equal(obs_output_get_proc_handler(&output), output.context.procs);
			assert_ptr_equal(obs_output_get_signal_handler(&output), output.context.signals);
			assert_ptr_equal(obs_output_get_proc_handler(&output), output.context.procs);
			assert_memory_equal(&output, &before, sizeof(output));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_handlers)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
