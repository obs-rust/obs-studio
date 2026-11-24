/* Characterization of obs_source_active() and obs_source_showing() (#5400),
 * the two source predicates every frontend, output and plugin uses to ask
 * whether a source is on program and whether it is shown anywhere.
 *
 * What it pins, on a test-owned struct obs_source (no core needed):
 *
 *   - NULL returns false and logs one LOG_DEBUG line naming the function
 *     that was called ("obs_source_active: Null 'source' parameter",
 *     "obs_source_showing: Null 'source' parameter");
 *   - a non-NULL source is never logged about;
 *   - each predicate is "its counter is nonzero": obs_source_active reads
 *     activate_refs only, obs_source_showing reads show_refs only, so every
 *     pair of counter values (zero, one, two, negative, the long extremes)
 *     gives two independent answers, with no other field consulted;
 *   - neither call writes to the source;
 *   - the counters are read atomically: a second thread changes both with
 *     os_atomic_inc_long/os_atomic_dec_long while this thread asks, as the
 *     video thread does in libobs, and the counters end balanced and both
 *     predicates false; a plain read there is a data race ThreadSanitizer reports
 *     in the sanitizer runs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "util/threading.h"

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

static void null_source_is_false_and_names_the_caller(void **state)
{
	(void)state;
	capture_start();
	bool active = obs_source_active(NULL);
	unsigned active_count = log_count;
	int active_level = log_level;
	char active_message[128];
	memcpy(active_message, log_message, sizeof(active_message));
	bool showing = obs_source_showing(NULL);
	capture_stop();

	assert_false(active);
	assert_false(showing);
	assert_int_equal(active_count, 1);
	assert_int_equal(active_level, LOG_DEBUG);
	assert_string_equal(active_message, "obs_source_active: Null 'source' parameter");
	assert_int_equal(log_count, 2);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_showing: Null 'source' parameter");
}

static void each_reads_only_its_own_counter(void **state)
{
	(void)state;
	const long edges[] = {0, 1, 2, -1, LONG_MAX, LONG_MIN, 0x5a5a};
	const size_t n = sizeof(edges) / sizeof(edges[0]);
	struct obs_source source;
	unsigned char snapshot[sizeof(source)];

	capture_start();
	for (size_t a = 0; a < n; a++) {
		for (size_t s = 0; s < n; s++) {
			/* Every other byte set, so a predicate reading any other field
			 * (an enabled flag, a removed flag, the context) can't pass
			 * by finding zero there. */
			memset(&source, 0xff, sizeof(source));
			source.activate_refs = edges[a];
			source.show_refs = edges[s];
			memcpy(snapshot, &source, sizeof(source));
			for (unsigned repeat = 0; repeat < 2; repeat++) {
				assert_int_equal(obs_source_active(&source), edges[a] != 0);
				assert_int_equal(obs_source_showing(&source), edges[s] != 0);
				assert_memory_equal(snapshot, &source, sizeof(source));
			}

			memset(&source, 0, sizeof(source));
			source.activate_refs = edges[a];
			source.show_refs = edges[s];
			memcpy(snapshot, &source, sizeof(source));
			assert_int_equal(obs_source_active(&source), edges[a] != 0);
			assert_int_equal(obs_source_showing(&source), edges[s] != 0);
			assert_memory_equal(snapshot, &source, sizeof(source));
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

#define FLIPS 20000

struct flipper {
	struct obs_source *source;
	volatile long started;
};

static void *flip_counters(void *param)
{
	struct flipper *flipper = param;
	os_atomic_set_long(&flipper->started, 1);
	for (unsigned i = 0; i < FLIPS; i++) {
		os_atomic_inc_long(&flipper->source->activate_refs);
		os_atomic_inc_long(&flipper->source->show_refs);
		os_atomic_dec_long(&flipper->source->show_refs);
		os_atomic_dec_long(&flipper->source->activate_refs);
	}
	return NULL;
}

static void reads_while_another_thread_changes_the_counts(void **state)
{
	(void)state;
	struct obs_source source;
	memset(&source, 0, sizeof(source));
	struct flipper flipper = {.source = &source, .started = 0};
	pthread_t thread;

	assert_int_equal(pthread_create(&thread, NULL, flip_counters, &flipper), 0);
	while (!os_atomic_load_long(&flipper.started))
		;
	unsigned active_seen = 0, showing_seen = 0;
	for (unsigned i = 0; i < FLIPS; i++) {
		active_seen += obs_source_active(&source) ? 1 : 0;
		showing_seen += obs_source_showing(&source) ? 1 : 0;
	}
	assert_int_equal(pthread_join(thread, NULL), 0);

	/* Whatever the interleaving, each answer counted at most once. */
	assert_true(active_seen <= FLIPS);
	assert_true(showing_seen <= FLIPS);
	assert_int_equal(os_atomic_load_long(&source.activate_refs), 0);
	assert_int_equal(os_atomic_load_long(&source.show_refs), 0);
	assert_false(obs_source_active(&source));
	assert_false(obs_source_showing(&source));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source_is_false_and_names_the_caller),
		cmocka_unit_test(each_reads_only_its_own_counter),
		cmocka_unit_test(reads_while_another_thread_changes_the_counts),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
