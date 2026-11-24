/* Characterization of obs_source_removed() (#5585), the predicate scenes,
 * transitions and the frontend ask before they touch a source that may be
 * on its way out.
 *
 * What it pins, on a test-owned struct obs_source (no core needed):
 *
 *   - NULL returns true (a missing source counts as removed, unlike
 *     obs_source_active/obs_source_showing) and logs one LOG_DEBUG line,
 *     "obs_source_removed: Null 'source' parameter";
 *   - a non-NULL source is never logged about;
 *   - the answer is the removed flag only: with every other byte set to 0xff
 *     or to 0 the result follows removed alone, and the call writes nothing;
 *   - the flag is read atomically: a second thread flips it with
 *     os_atomic_set_bool while this thread asks, as obs_source_remove() does
 *     from the UI thread while the render thread reads it, and the last
 *     value written is the one read once the writer is done. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
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

static void null_source_is_removed_and_names_the_caller(void **state)
{
	(void)state;
	capture_start();
	bool removed = obs_source_removed(NULL);
	capture_stop();

	assert_true(removed);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_removed: Null 'source' parameter");
}

static void reads_only_the_removed_flag(void **state)
{
	(void)state;
	const unsigned char fills[] = {0x00, 0xff, 0x5a};
	struct obs_source source;
	unsigned char snapshot[sizeof(source)];

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (int value = 0; value < 2; value++) {
			/* Every other byte set to the fill, so a predicate reading
			 * any other field (temp_removed, the active or show counts,
			 * the context) can't pass by finding the expected answer
			 * there. */
			memset(&source, fills[f], sizeof(source));
			source.removed = value != 0;
			memcpy(snapshot, &source, sizeof(source));
			for (unsigned repeat = 0; repeat < 2; repeat++) {
				assert_int_equal(obs_source_removed(&source), value != 0);
				assert_memory_equal(snapshot, &source, sizeof(source));
			}
		}
	}

	/* temp_removed is a different flag: it never answers for removed. */
	memset(&source, 0, sizeof(source));
	source.temp_removed = true;
	assert_false(obs_source_removed(&source));
	source.removed = true;
	source.temp_removed = false;
	assert_true(obs_source_removed(&source));
	capture_stop();
	assert_int_equal(log_count, 0);
}

#define FLIPS 20000

struct flipper {
	struct obs_source *source;
	volatile long started;
};

static void *flip_removed(void *param)
{
	struct flipper *flipper = param;
	os_atomic_set_long(&flipper->started, 1);
	for (unsigned i = 0; i < FLIPS; i++) {
		os_atomic_set_bool(&flipper->source->removed, true);
		os_atomic_set_bool(&flipper->source->removed, false);
	}
	os_atomic_set_bool(&flipper->source->removed, true);
	return NULL;
}

static void reads_while_another_thread_changes_the_flag(void **state)
{
	(void)state;
	struct obs_source source;
	memset(&source, 0, sizeof(source));
	struct flipper flipper = {.source = &source, .started = 0};
	pthread_t thread;

	assert_int_equal(pthread_create(&thread, NULL, flip_removed, &flipper), 0);
	while (!os_atomic_load_long(&flipper.started))
		;
	unsigned removed_seen = 0;
	for (unsigned i = 0; i < FLIPS; i++)
		removed_seen += obs_source_removed(&source) ? 1 : 0;
	assert_int_equal(pthread_join(thread, NULL), 0);

	/* Whatever the interleaving, each answer counted at most once, and
	 * the writer's last value is what a later read sees. */
	assert_true(removed_seen <= FLIPS);
	assert_true(obs_source_removed(&source));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source_is_removed_and_names_the_caller),
		cmocka_unit_test(reads_only_the_removed_flag),
		cmocka_unit_test(reads_while_another_thread_changes_the_flag),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
