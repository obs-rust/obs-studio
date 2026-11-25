/* Characterization of obs_source_inc_showing(), obs_source_inc_active(),
 * obs_source_dec_showing() and obs_source_dec_active() (#5601), the four
 * public counters frontends, projectors and plugins use to mark a source as
 * shown somewhere or as on program.
 *
 * What it pins, on a test-owned struct obs_source (no core needed; its
 * context.data is NULL, so the active-tree walk that follows each counter
 * change finds no children and touches nothing):
 *
 *   - NULL changes nothing and logs one LOG_DEBUG line naming the function
 *     that was called ("obs_source_inc_showing: Null 'source' parameter",
 *     and so on for each of the four);
 *   - a non-NULL source is never logged about;
 *   - inc_showing raises show_refs by one and nothing else;
 *   - inc_active raises show_refs and activate_refs by one each;
 *   - dec_showing lowers show_refs by one when it is above zero and leaves it
 *     otherwise, and never touches activate_refs;
 *   - dec_active lowers each of show_refs and activate_refs by one when that
 *     counter is above zero, each independently of the other;
 *   - no call writes any other byte of the source;
 *   - a balanced sequence ends at zero, and extra decrements keep it there. */
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

typedef void (*counter_call)(obs_source_t *source);

struct counter_case {
	const char *name;
	counter_call call;
	/* What the call does to a counter above zero: +1, -1. */
	long show_step;
	long activate_step;
};

static const struct counter_case cases[] = {
	{"obs_source_inc_showing", obs_source_inc_showing, 1, 0},
	{"obs_source_inc_active", obs_source_inc_active, 1, 1},
	{"obs_source_dec_showing", obs_source_dec_showing, -1, 0},
	{"obs_source_dec_active", obs_source_dec_active, -1, -1},
};
#define CASES (sizeof(cases) / sizeof(cases[0]))

/* A decrement only applies above zero; an increment always applies. */
static long expected(long value, long step)
{
	if (step < 0 && value <= 0)
		return value;
	return value + step;
}

static void null_source_changes_nothing_and_names_the_caller(void **state)
{
	(void)state;
	for (size_t i = 0; i < CASES; i++) {
		char want[128];
		snprintf(want, sizeof(want), "%s: Null 'source' parameter", cases[i].name);
		capture_start();
		cases[i].call(NULL);
		capture_stop();
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(log_message, want);
	}
}

static void each_moves_only_its_counters(void **state)
{
	(void)state;
	const long edges[] = {0, 1, 2, -1, -2, 0x5a5a};
	const size_t n = sizeof(edges) / sizeof(edges[0]);
	const unsigned char noises[] = {0x00, 0x5a, 0xff};
	struct obs_source source;
	struct obs_source want;
	unsigned calls = 0;

	capture_start();
	for (size_t c = 0; c < CASES; c++) {
		for (size_t k = 0; k < sizeof(noises); k++) {
			for (size_t s = 0; s < n; s++) {
				for (size_t a = 0; a < n; a++) {
					/* Every other byte set, so a write anywhere else
					 * shows; context.data NULL keeps the tree walk
					 * out of the noise. */
					memset(&source, noises[k], sizeof(source));
					source.context.data = NULL;
					source.show_refs = edges[s];
					source.activate_refs = edges[a];
					memcpy(&want, &source, sizeof(source));
					want.show_refs = expected(edges[s], cases[c].show_step);
					want.activate_refs = expected(edges[a], cases[c].activate_step);

					cases[c].call(&source);
					calls++;

					assert_int_equal(source.show_refs, want.show_refs);
					assert_int_equal(source.activate_refs, want.activate_refs);
					assert_memory_equal(&want, &source, sizeof(source));
				}
			}
		}
	}
	capture_stop();
	assert_int_equal(calls, CASES * sizeof(noises) * n * n);
	assert_int_equal(log_count, 0);
}

static void balanced_sequence_ends_at_zero(void **state)
{
	(void)state;
	struct obs_source source;
	memset(&source, 0, sizeof(source));

	capture_start();
	obs_source_inc_active(&source);
	obs_source_inc_showing(&source);
	obs_source_inc_active(&source);
	assert_int_equal(source.show_refs, 3);
	assert_int_equal(source.activate_refs, 2);

	/* dec_showing leaves the program count alone. */
	obs_source_dec_showing(&source);
	assert_int_equal(source.show_refs, 2);
	assert_int_equal(source.activate_refs, 2);

	obs_source_dec_active(&source);
	obs_source_dec_active(&source);
	assert_int_equal(source.show_refs, 0);
	assert_int_equal(source.activate_refs, 0);

	/* Past zero, nothing moves either way. */
	obs_source_dec_active(&source);
	obs_source_dec_showing(&source);
	assert_int_equal(source.show_refs, 0);
	assert_int_equal(source.activate_refs, 0);

	/* A shown-only source: dec_active takes the show count down and keeps
	 * the program count at zero. */
	obs_source_inc_showing(&source);
	obs_source_dec_active(&source);
	assert_int_equal(source.show_refs, 0);
	assert_int_equal(source.activate_refs, 0);
	capture_stop();
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source_changes_nothing_and_names_the_caller),
		cmocka_unit_test(each_moves_only_its_counters),
		cmocka_unit_test(balanced_sequence_ends_at_zero),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
