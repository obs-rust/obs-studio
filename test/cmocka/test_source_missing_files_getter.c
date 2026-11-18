/* Characterize the public callback/fallback dispatch before its Rust swap (Refs #5085). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include "obs-internal.h"

#if !__has_include("obs-source-missing-files-getter.c.h")
#error "source missing-files getter extraction seam is missing"
#endif

static unsigned log_count;
static int log_level;
static char message[256];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(message, sizeof(message), format, args);
}

struct callback_state {
	unsigned calls;
	void *received;
	obs_missing_files_t *result;
};

static obs_missing_files_t *missing_files(void *data)
{
	struct callback_state *state = data;
	state->calls++;
	state->received = data;
	return state->result;
}

static obs_missing_files_t *unexpected_missing_files(void *data)
{
	(void)data;
	fail();
	return NULL;
}

static void fallback(const obs_source_t *source, unsigned expected_logs)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	log_count = 0;
	base_set_log_handler(capture, NULL);
	obs_missing_files_t *first = obs_source_get_missing_files(source);
	obs_missing_files_t *second = obs_source_get_missing_files(source);
	base_set_log_handler(previous, param);
	assert_non_null(first);
	assert_non_null(second);
	assert_ptr_not_equal(first, second);
	assert_int_equal(obs_missing_files_count(first), 0);
	assert_int_equal(obs_missing_files_count(second), 0);
	obs_missing_files_destroy(first);
	obs_missing_files_destroy(second);
	assert_int_equal(log_count, expected_logs);
	if (expected_logs) {
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(message, "obs_source_get_missing_files: Null 'source' parameter");
	}
}

static void null_source(void **unused)
{
	(void)unused;
	fallback(NULL, 2);
}

static void null_data_skips_callback(void **unused)
{
	(void)unused;
	/* This getter reads only these fields; the synthetic source owns no state. */
	struct obs_source source = {0};
	source.info.missing_files = unexpected_missing_files;
	fallback(&source, 0);
}

static void absent_callback(void **unused)
{
	(void)unused;
	struct callback_state state = {0};
	struct obs_source source = {0};
	source.context.data = &state;
	fallback(&source, 0);
	assert_int_equal(state.calls, 0);
}

static void callback_result(void **unused)
{
	(void)unused;
	struct callback_state state = {.result = obs_missing_files_create()};
	struct obs_source source = {0};
	source.context.data = &state;
	source.info.missing_files = missing_files;
	obs_missing_files_t *result = obs_source_get_missing_files(&source);
	assert_ptr_equal(result, state.result);
	assert_int_equal(state.calls, 1);
	assert_ptr_equal(state.received, &state);
	obs_missing_files_destroy(result);
}

static void null_callback_result(void **unused)
{
	(void)unused;
	struct callback_state state = {0};
	struct obs_source source = {0};
	source.context.data = &state;
	source.info.missing_files = missing_files;
	/* A callback-returned NULL is preserved: characterized, not endorsed. */
	assert_null(obs_source_get_missing_files(&source));
	assert_int_equal(state.calls, 1);
	assert_ptr_equal(state.received, &state);
}

int main(void)
{
	/* No OBS startup, graphics, audio, threads, or source destruction is needed. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),          cmocka_unit_test(null_data_skips_callback),
		cmocka_unit_test(absent_callback),      cmocka_unit_test(callback_result),
		cmocka_unit_test(null_callback_result),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
