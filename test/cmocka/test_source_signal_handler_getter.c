/* Characterization of obs_source_get_signal_handler() (#5591), the borrowed
 * signal handler that scenes, transitions, plugins and the frontend connect
 * their callbacks to.
 *
 * What it pins, on a test-owned struct obs_source (no core needed):
 *
 *   - NULL returns NULL and logs one LOG_DEBUG line,
 *     "obs_source_get_signal_handler: Null 'source' parameter";
 *   - a non-NULL source is never logged about;
 *   - the answer is context.signals itself: the same pointer (borrowed, no
 *     reference taken), NULL when the source has none, whatever procs and
 *     every other byte hold, and the call writes nothing to the source. */
#include <stdarg.h>
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

static void null_source_answers_null_and_names_the_caller(void **state)
{
	(void)state;
	capture_start();
	signal_handler_t *signals = obs_source_get_signal_handler(NULL);
	capture_stop();

	assert_null(signals);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_get_signal_handler: Null 'source' parameter");
}

static void answers_the_borrowed_signal_handler(void **state)
{
	(void)state;
	const unsigned char fills[] = {0x00, 0xff, 0x5a};
	struct obs_source source;
	unsigned char snapshot[sizeof(source)];
	/* Borrowed addresses only; neither handler token is dereferenced. */
	_Alignas(long double) unsigned char signal_token = 0, proc_token = 0;
	signal_handler_t *signals = (signal_handler_t *)&signal_token;
	proc_handler_t *procs = (proc_handler_t *)&proc_token;

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (unsigned s = 0; s < 2; s++) {
			for (unsigned p = 0; p < 3; p++) {
				/* Every other byte set to the fill, so a getter reading
				 * any other field (procs among them) can't pass by
				 * finding the expected answer there. p == 2 puts the
				 * signal handler's own address in procs. */
				memset(&source, fills[f], sizeof(source));
				source.context.signals = s ? signals : NULL;
				source.context.procs = p == 0   ? NULL
						       : p == 1 ? procs
								: (proc_handler_t *)(void *)signals;
				memcpy(snapshot, &source, sizeof(source));
				for (unsigned repeat = 0; repeat < 2; repeat++) {
					assert_ptr_equal(obs_source_get_signal_handler(&source), s ? signals : NULL);
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
		cmocka_unit_test(null_source_answers_null_and_names_the_caller),
		cmocka_unit_test(answers_the_borrowed_signal_handler),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
