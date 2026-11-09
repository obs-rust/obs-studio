/* Actual-library characterization of the atomic setter and signal dispatch. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
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

static void null_is_logged(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	obs_source_set_audio_active(NULL, true);
	obs_source_set_audio_active(NULL, false);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(log_count, 2);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_set_audio_active: Null 'source' parameter");
}

struct observation {
	obs_source_t *source;
	unsigned events[8];
	unsigned count;
	bool active;
};

struct listener {
	struct observation *observation;
	unsigned event;
};

static void observe(void *param, calldata_t *data)
{
	struct listener *listener = param;
	struct observation *observation = listener->observation;
	assert_ptr_equal(calldata_ptr(data, "source"), observation->source);
	/* The exchange is observable before either callback is invoked. */
	assert_int_equal(obs_source_audio_active(observation->source), observation->active);
	assert_true(observation->count < 8);
	observation->events[observation->count++] = listener->event;
}

static void state_and_signals(void **state)
{
	(void)state;
	signal_handler_t *global = obs_get_signal_handler();
	for (unsigned private = 0; private < 2; private++) {
		for (unsigned previous = 0; previous < 2; previous++) {
			for (unsigned active = 0; active < 2; active++) {
				struct obs_source source = {0};
				source.context.private = private != 0;
				source.context.signals = signal_handler_create();
				assert_non_null(source.context.signals);
				assert_true(
					signal_handler_add(source.context.signals, "void audio_activate(ptr source)"));
				assert_true(signal_handler_add(source.context.signals,
							       "void audio_deactivate(ptr source)"));
				os_atomic_store_bool(&source.audio_active, previous != 0);
				struct observation observation = {.source = &source, .active = active != 0};
				struct listener listeners[] = {{&observation, 1},
							       {&observation, 2},
							       {&observation, 3},
							       {&observation, 4}};
				signal_handler_connect(global, "source_audio_activate", observe, &listeners[0]);
				signal_handler_connect(source.context.signals, "audio_activate", observe,
						       &listeners[1]);
				signal_handler_connect(global, "source_audio_deactivate", observe, &listeners[2]);
				signal_handler_connect(source.context.signals, "audio_deactivate", observe,
						       &listeners[3]);

				obs_source_set_audio_active(&source, active != 0);
				assert_int_equal(os_atomic_load_bool(&source.audio_active), active);
				unsigned expected = previous == active ? 0 : (private ? 1 : 2);
				assert_int_equal(observation.count, expected);
				if (expected) {
					unsigned first = active ? 1 : 3;
					assert_int_equal(observation.events[0], first + private);
					if (!private)
						assert_int_equal(observation.events[1], first + 1);
				}
				/* Repeating a value does not emit either signal. */
				obs_source_set_audio_active(&source, active != 0);
				assert_int_equal(observation.count, expected);
				assert_int_equal(os_atomic_load_bool(&source.audio_active), active);

				signal_handler_disconnect(global, "source_audio_activate", observe, &listeners[0]);
				signal_handler_disconnect(global, "source_audio_deactivate", observe, &listeners[2]);
				signal_handler_destroy(source.context.signals);
			}
		}
	}
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_is_logged), cmocka_unit_test(state_and_signals)};
	return cmocka_run_group_tests(tests, setup, teardown);
}
