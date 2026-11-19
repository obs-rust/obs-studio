/* Actual-library characterization of obs_output_get_name() before its body
 * moves out of obs-output.c.h (obs-rust/obs-studio#5300). It pins the NULL
 * path with its exact debug log and NULL result, and that a valid output
 * returns its own context name pointer without logging: the given name, the
 * name generated for an empty one, and a changed context-name pointer.
 * Outputs have no public name setter; test_context_names keeps its own naming cases.
 *
 * libobs needs an X display on Linux, so the cases skip without one (run
 * under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <obs-internal.h>

#define OUTPUT_ID "test_get_name_output"

static log_handler_t old_handler;
static void *old_param;
static unsigned log_count;
static int log_level;
static char log_message[256];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void start_capture(void)
{
	base_get_log_handler(&old_handler, &old_param);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
	base_set_log_handler(capture_log, NULL);
}

static void stop_capture(void)
{
	base_set_log_handler(old_handler, old_param);
}

static const char *out_name(void *type_data)
{
	(void)type_data;
	return "get name output";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return bzalloc(1);
}

static void out_destroy(void *data)
{
	bfree(data);
}

static bool out_start(void *data)
{
	(void)data;
	return false;
}

static void out_stop(void *data, uint64_t timestamp)
{
	(void)data;
	(void)timestamp;
}

static void out_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static bool obs_up;

static int setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_output_info info = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_ENCODED,
		.get_name = out_name,
		.create = out_create,
		.destroy = out_destroy,
		.start = out_start,
		.stop = out_stop,
		.encoded_packet = out_packet,
	};
	obs_register_output(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

static void null_output_logs_and_returns_null(void **state)
{
	(void)state;
	start_capture();
	const char *name = obs_output_get_name(NULL);
	stop_capture();

	assert_null(name);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_output_get_name: Null 'output' parameter");
}

static void returns_the_context_name_pointer_silently(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_output_t *output = obs_output_create(OUTPUT_ID, "get name", NULL, NULL);
	assert_non_null(output);

	start_capture();
	const char *name = obs_output_get_name(output);
	const char *again = obs_output_get_name(output);
	stop_capture();

	assert_int_equal(log_count, 0);
	assert_ptr_equal(name, output->context.name);
	assert_ptr_equal(again, name);
	assert_string_equal(name, "get name");

	obs_output_release(output);
}

static void unnamed_output_returns_its_generated_name(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_output_t *output = obs_output_create(OUTPUT_ID, "", NULL, NULL);
	assert_non_null(output);

	start_capture();
	const char *name = obs_output_get_name(output);
	stop_capture();

	assert_int_equal(log_count, 0);
	assert_non_null(name);
	assert_ptr_equal(name, output->context.name);
	assert_int_equal(strncmp(name, "__unnamed", 9), 0);

	obs_output_release(output);
}

static void reads_the_current_context_name(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_output_t *output = obs_output_create(OUTPUT_ID, "before", NULL, NULL);
	assert_non_null(output);
	assert_string_equal(obs_output_get_name(output), "before");

	/* Outputs have no public rename API. Restore the registered name before
	 * destruction so the context hash and its allocation stay untouched. */
	char *registered_name = output->context.name;
	char replacement[] = "after";
	output->context.name = replacement;

	start_capture();
	const char *name = obs_output_get_name(output);
	stop_capture();
	output->context.name = registered_name;

	assert_int_equal(log_count, 0);
	assert_ptr_equal(name, replacement);
	assert_string_equal(name, "after");

	obs_output_release(output);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_output_logs_and_returns_null),
		cmocka_unit_test(returns_the_context_name_pointer_silently),
		cmocka_unit_test(unnamed_output_returns_its_generated_name),
		cmocka_unit_test(reads_the_current_context_name),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
