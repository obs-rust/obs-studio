/* Characterize obs_source_configurable() before its later Rust swap (Refs #5092). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-source-configurable.c.h")
#error "source configurable query extraction seam is missing"
#endif

static unsigned log_count;
static int log_level;
static char log_message[128];
static unsigned callback_calls;

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

static obs_properties_t *get_properties(void *data)
{
	(void)data;
	callback_calls++;
	return NULL;
}

static obs_properties_t *get_properties2(void *data, void *type_data)
{
	(void)data;
	(void)type_data;
	callback_calls++;
	return NULL;
}

/* Calls obs_source_configurable() with logging captured. */
static bool configurable(const obs_source_t *source)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	bool result = obs_source_configurable(source);
	base_set_log_handler(old_handler, old_param);
	return result;
}

static void null_source(void **state)
{
	(void)state;
	log_count = 0;
	assert_false(configurable(NULL));
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_configurable: Null 'source' parameter");
}

/* Without instance data the answer is false, whatever callbacks exist, and
 * nothing is logged. */
static void no_data(void **state)
{
	(void)state;
	struct obs_source source = {0};
	source.info.get_properties = get_properties;
	source.info.get_properties2 = get_properties2;
	log_count = 0;
	callback_calls = 0;
	assert_false(configurable(&source));
	source.info.get_properties = NULL;
	assert_false(configurable(&source));
	source.info.get_properties = get_properties;
	source.info.get_properties2 = NULL;
	assert_false(configurable(&source));
	assert_int_equal(log_count, 0);
	assert_int_equal(callback_calls, 0);
}

/* With data, either callback makes the source configurable; neither is
 * called, and the source is not changed. */
static void callback_presence(void **state)
{
	(void)state;
	int data = 0;
	struct obs_source source = {0};
	unsigned char snapshot[sizeof(source)];
	source.context.data = &data;
	log_count = 0;
	callback_calls = 0;
	for (int first = 0; first < 2; first++) {
		for (int second = 0; second < 2; second++) {
			source.info.get_properties = first ? get_properties : NULL;
			source.info.get_properties2 = second ? get_properties2 : NULL;
			memcpy(snapshot, &source, sizeof(source));
			assert_int_equal(configurable(&source), first || second);
			assert_memory_equal(snapshot, &source, sizeof(source));
		}
	}
	assert_int_equal(log_count, 0);
	assert_int_equal(callback_calls, 0);
}

/* The query reads the source's own info copy, not the type registry: an id
 * that was never registered answers from its own callbacks. */
static void reads_own_info(void **state)
{
	(void)state;
	int data = 0;
	struct obs_source source = {0};
	source.context.data = &data;
	source.info.id = "test_source_configurable_never_registered";
	callback_calls = 0;
	assert_false(configurable(&source));
	source.info.get_properties2 = get_properties2;
	assert_true(configurable(&source));
	assert_int_equal(callback_calls, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test(no_data),
		cmocka_unit_test(callback_presence),
		cmocka_unit_test(reads_own_info),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
