/* Characterize actual-library atomic encoder state queries. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <stdio.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[128];

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

static void null_defaults(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	bool active = obs_encoder_active(NULL);
	unsigned after_active = log_count;
	char active_message[128];
	memcpy(active_message, log_message, sizeof(active_message));
	bool paused = obs_encoder_paused(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_false(active);
	assert_false(paused);
	assert_int_equal(after_active, 1);
	assert_int_equal(log_count, 2);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(active_message, "obs_encoder_active: Null 'encoder' parameter");
	assert_string_equal(log_message, "obs_encoder_paused: Null 'encoder' parameter");
}

static void independent_states(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	encoder.initialized = true;
	for (unsigned active = 0; active < 2; active++) {
		for (unsigned paused = 0; paused < 2; paused++) {
			os_atomic_set_bool(&encoder.active, active != 0);
			os_atomic_set_bool(&encoder.paused, paused != 0);
			assert_int_equal(obs_encoder_active(&encoder), active);
			assert_int_equal(obs_encoder_paused(&encoder), paused);
			assert_int_equal(os_atomic_load_bool(&encoder.active), active);
			assert_int_equal(os_atomic_load_bool(&encoder.paused), paused);
			assert_true(encoder.initialized);
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults), cmocka_unit_test(independent_states)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
