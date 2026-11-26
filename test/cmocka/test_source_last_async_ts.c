/* Characterization of obs_source_get_last_async_ts() (#5766), the getter the
 * source profiler reads each tick for an async video source.
 *
 * The getter is hidden in libobs (obs-internal.h, no EXPORT), so this test
 * compiles its extracted C file into itself: it pins the C source only, as
 * test_source_texcoords_centered_setter does. source-profiler.c.h, the only
 * caller, reaches the swapped getter through hybrid libobs.
 *
 * What it pins, on a test-owned struct obs_source (no core needed):
 *
 *   - obs_source_get_last_async_ts(source) answers async_last_rendered_ts
 *     exactly, for 0, ordinary timestamps, single bits and UINT64_MAX,
 *     whatever every other byte of the source holds;
 *   - it writes no byte of the source and logs nothing, however often it is
 *     asked.
 *
 * There is no NULL check in the C: NULL is outside the contract and is not
 * tested. */
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "libobs/build/libobs.pch.h"
#include "libobs/obs-source-async-last-ts-getter.c.h"

static unsigned log_count;
static void count_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	log_count++;
}

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(count_log, NULL);
	log_count = 0;
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

static const unsigned char fills[] = {0x00, 0xff, 0x5a};

static const uint64_t values[] = {
	0,
	1,
	33333333,
	1000000000ull * 60 * 60 * 24,
	0x8000000000000000ull,
	0x7fffffffffffffffull,
	0x0123456789abcdefull,
	UINT64_MAX,
};

static void answers_the_stored_timestamp(void **state)
{
	(void)state;
	static struct obs_source source;

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (size_t v = 0; v < sizeof(values) / sizeof(values[0]); v++) {
			memset(&source, fills[f], sizeof(source));
			source.async_last_rendered_ts = values[v];
			assert_true(obs_source_get_last_async_ts(&source) == values[v]);
		}
		/* Every single bit, so a getter that masks or truncates fails. */
		for (unsigned bit = 0; bit < 64; bit++) {
			memset(&source, fills[f], sizeof(source));
			source.async_last_rendered_ts = UINT64_C(1) << bit;
			assert_true(obs_source_get_last_async_ts(&source) == UINT64_C(1) << bit);
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void writes_nothing(void **state)
{
	(void)state;
	static struct obs_source source;
	static unsigned char snapshot[sizeof(source)];

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (size_t v = 0; v < sizeof(values) / sizeof(values[0]); v++) {
			/* The neighbouring timing fields hold other values, so a getter
			 * reading the wrong field fails. */
			memset(&source, fills[f], sizeof(source));
			source.async_last_rendered_ts = values[v];
			source.last_frame_ts = ~values[v];
			source.last_sys_timestamp = values[v] ^ 0x5555555555555555ull;
			memcpy(snapshot, &source, sizeof(source));
			for (unsigned repeat = 0; repeat < 3; repeat++) {
				assert_true(obs_source_get_last_async_ts(&source) == values[v]);
				assert_memory_equal(snapshot, &source, sizeof(source));
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(answers_the_stored_timestamp),
		cmocka_unit_test(writes_nothing),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
