/* Characterize borrowed source procedure handler through the actual library exports. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void borrowed_handlers(void **state)
{
	(void)state;
	struct obs_source source = {0}, before;
	/* Borrowed addresses only; neither handler token is dereferenced. */
	_Alignas(long double) unsigned char signal_token = 0, proc_token = 0;
	signal_handler_t *signals = (signal_handler_t *)&signal_token;
	proc_handler_t *procs = (proc_handler_t *)&proc_token;
	assert_null(obs_source_get_proc_handler(NULL));
	for (unsigned s = 0; s < 2; s++) {
		for (unsigned p = 0; p < 2; p++) {
			source.context.signals = s ? signals : NULL;
			source.context.procs = p ? procs : NULL;
			memcpy(&before, &source, sizeof(source));
			assert_ptr_equal(obs_source_get_proc_handler(&source), source.context.procs);
			assert_ptr_equal(obs_source_get_proc_handler(&source), source.context.procs);
			assert_memory_equal(&source, &before, sizeof(source));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_handlers)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
