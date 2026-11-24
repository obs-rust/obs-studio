/* Characterization of obs_source_set_async_unbuffered() and
 * obs_source_async_compensation() (#5595), the async flag accessors whose
 * siblings (obs_source_async_unbuffered() and
 * obs_source_set_async_compensation()) are already Rust.
 *
 * What it pins, on a test-owned struct obs_source (no core needed):
 *
 *   - obs_source_set_async_unbuffered(NULL, x) logs one LOG_DEBUG line,
 *     "obs_source_set_async_unbuffered: Null 'source' parameter", for both
 *     values of x;
 *   - otherwise it stores x into async_unbuffered and writes no other byte,
 *     whatever every other byte holds, and logs nothing;
 *   - obs_source_async_compensation(NULL) answers false and logs one
 *     LOG_DEBUG line, "obs_source_async_compensation: Null 'source'
 *     parameter";
 *   - otherwise it answers async_compensation itself, whatever every other
 *     byte holds (async_unbuffered and own_clock_compensation among them),
 *     writes nothing and logs nothing. */
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <setjmp.h>
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

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

static const unsigned char fills[] = {0x00, 0xff, 0x5a};

static void set_unbuffered_null_logs_the_caller(void **state)
{
	(void)state;
	for (unsigned value = 0; value < 2; value++) {
		capture_start();
		obs_source_set_async_unbuffered(NULL, value != 0);
		capture_stop();

		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(log_message, "obs_source_set_async_unbuffered: Null 'source' parameter");
	}
}

static void set_unbuffered_stores_only_the_flag(void **state)
{
	(void)state;
	static struct obs_source source;
	static unsigned char expected[sizeof(source)];
	const size_t at = offsetof(struct obs_source, async_unbuffered);

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		/* Every other byte holds the fill, and the flag starts at each
		 * value too, so a setter writing any neighbour (async_decoupled
		 * among them) or skipping the store shows up in the bytes. */
		for (unsigned initial = 0; initial < 2; initial++) {
			const bool sequence[] = {true, true, false, false, true, false};
			memset(&source, fills[f], sizeof(source));
			source.async_unbuffered = initial != 0;
			memcpy(expected, &source, sizeof(source));
			for (size_t i = 0; i < sizeof(sequence) / sizeof(sequence[0]); i++) {
				obs_source_set_async_unbuffered(&source, sequence[i]);
				memcpy(expected + at, &sequence[i], sizeof(bool));
				assert_memory_equal(expected, &source, sizeof(source));
				assert_int_equal(source.async_unbuffered, sequence[i]);
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void compensation_null_answers_false_and_logs(void **state)
{
	(void)state;
	capture_start();
	bool compensation = obs_source_async_compensation(NULL);
	capture_stop();

	assert_false(compensation);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_async_compensation: Null 'source' parameter");
}

static void compensation_answers_the_flag(void **state)
{
	(void)state;
	static struct obs_source source;
	static unsigned char snapshot[sizeof(source)];

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (unsigned value = 0; value < 2; value++) {
			for (unsigned others = 0; others < 2; others++) {
				/* The neighbouring flags hold the opposite answer or the
				 * same one, so reading any of them instead fails. */
				memset(&source, fills[f], sizeof(source));
				source.async_compensation = value != 0;
				source.last_async_compensation = (others != 0) != (value != 0);
				source.own_clock_compensation = (others != 0) != (value != 0);
				source.async_unbuffered = (others != 0) != (value != 0);
				source.async_decoupled = (others != 0) != (value != 0);
				memcpy(snapshot, &source, sizeof(source));
				for (unsigned repeat = 0; repeat < 2; repeat++) {
					assert_int_equal(obs_source_async_compensation(&source), value != 0);
					assert_memory_equal(snapshot, &source, sizeof(source));
				}
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(set_unbuffered_null_logs_the_caller),
		cmocka_unit_test(set_unbuffered_stores_only_the_flag),
		cmocka_unit_test(compensation_null_answers_false_and_logs),
		cmocka_unit_test(compensation_answers_the_flag),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
