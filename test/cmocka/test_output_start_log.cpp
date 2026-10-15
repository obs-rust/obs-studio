// The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
// with libstdc++.
#include <cstdio>
#include <cstring>
#include <string>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

#include <util/base.h>
#include <util/c99defs.h>

#include "output-start-log.hpp"

/* obs-rust/obs-studio#817: a start of streaming, recording, the replay buffer
 * or the virtual camera that the frontend refused because the outputs were
 * disabled (Settings dialog or auto-configuration wizard open) left no trace.
 * obs-websocket replies success to the start request, so the log is the only
 * place where the refusal can show. */

static int last_level = -1;
static std::string last_message;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	UNUSED_PARAMETER(param);
	char buffer[1024];
	vsnprintf(buffer, sizeof(buffer), format, args);
	last_level = level;
	last_message = buffer;
}

static log_handler_t previous_handler;
static void *previous_param;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	base_get_log_handler(&previous_handler, &previous_param);
	base_set_log_handler(capture_log, nullptr);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	base_set_log_handler(previous_handler, previous_param);
	return 0;
}

static void refused_start_logs_a_warning(void **state)
{
	UNUSED_PARAMETER(state);
	last_level = -1;
	last_message.clear();

	LogOutputStartRefused("Recording");

	assert_int_equal(last_level, LOG_WARNING);
	assert_non_null(strstr(last_message.c_str(), "Recording not started"));
}

static void warning_names_the_reason(void **state)
{
	UNUSED_PARAMETER(state);
	last_message.clear();

	LogOutputStartRefused("Streaming");

	assert_non_null(strstr(last_message.c_str(), "outputs are disabled"));
	assert_non_null(strstr(last_message.c_str(), "Settings dialog"));
	assert_non_null(strstr(last_message.c_str(), "Auto-Configuration Wizard"));
}

int main()
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(refused_start_logs_a_warning),
		cmocka_unit_test(warning_names_the_reason),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
