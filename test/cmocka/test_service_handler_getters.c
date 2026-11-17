/* C-source characterization of the hidden service handler getters (#3840).
 * Neither symbol is declared by a public header or exported from libobs.
 * Compile the real extracted implementation; this fixture does not test a
 * future Rust replacement until that port updates its link registration. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#ifndef OBS_SERVICE_HANDLER_GETTERS_RUST
#include "../../libobs/obs-service-handler-getters.c.h"
#else
/* The later Rust-static registration selects this without editing this test. */
signal_handler_t *obs_service_get_signal_handler(const obs_service_t *service);
proc_handler_t *obs_service_get_proc_handler(const obs_service_t *service);
#endif

struct log_capture {
	unsigned count;
	const char *function;
	const char *parameter;
};

static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct log_capture *capture = param;
	assert_int_equal(level, LOG_DEBUG);
	assert_string_equal(format, "%s: Null '%s' parameter");
	capture->count++;
	capture->function = va_arg(args, const char *);
	capture->parameter = va_arg(args, const char *);
}

static void borrowed_handlers(void **state)
{
	(void)state;
	struct obs_service service = {0}, before;
	/* Borrowed tokens only; the getter never dereferences either address. */
	_Alignas(long double) unsigned char signal_token = 0, proc_token = 0;
	signal_handler_t *signals = (signal_handler_t *)&signal_token;
	proc_handler_t *procs = (proc_handler_t *)&proc_token;
	struct log_capture capture = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, &capture);

	assert_null(obs_service_get_signal_handler(NULL));
	assert_int_equal(capture.count, 1);
	assert_string_equal(capture.function, "obs_service_get_signal_handler");
	assert_string_equal(capture.parameter, "service");
	assert_null(obs_service_get_proc_handler(NULL));
	assert_int_equal(capture.count, 2);
	assert_string_equal(capture.function, "obs_service_get_proc_handler");
	assert_string_equal(capture.parameter, "service");

	for (unsigned s = 0; s < 2; s++) {
		for (unsigned p = 0; p < 2; p++) {
			service.context.signals = s ? signals : NULL;
			service.context.procs = p ? procs : NULL;
			memcpy(&before, &service, sizeof(service));
			assert_ptr_equal(obs_service_get_signal_handler(&service), service.context.signals);
			assert_ptr_equal(obs_service_get_proc_handler(&service), service.context.procs);
			assert_ptr_equal(obs_service_get_signal_handler(&service), service.context.signals);
			assert_ptr_equal(obs_service_get_proc_handler(&service), service.context.procs);
			assert_memory_equal(&service, &before, sizeof(service));
			assert_int_equal(capture.count, 2);
		}
	}
	base_set_log_handler(old_handler, old_param);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_handlers)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
