/* obs-rust/obs-studio#1284: waiting for an rtmp-services ingest refresh
 * (service-ingest and dacast) ends when the refresh attempt ends, not at a
 * 10 ms polling tick or at the timeout.
 *
 * obs-rust/obs-studio#2164: when its event cannot be created, the refresh
 * and the wait go on without it instead of using a NULL event. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <util/platform.h>
#include <util/threading.h>

#include "plugins/rtmp-services/service-specific/ingest-refresh-wait.h"

struct fake_refresh {
	struct ingest_refresh_wait *wait;
	unsigned long delay_ms;
	bool success;
};

static void *fake_refresh_thread(void *data)
{
	struct fake_refresh *f = data;
	os_sleep_ms((uint32_t)f->delay_ms);
	ingest_refresh_wait_complete(f->wait, f->success);
	return NULL;
}

/* Upper bounds are against the 5000 ms timeout the old polling loop waited
 * out, not against the 50 ms refresh: a loaded CI runner (macOS) can
 * oversleep a 50 ms os_sleep_ms well past 150 ms. */
static uint64_t run_wait(bool success, unsigned long delay_ms, unsigned long timeout_ms, bool *ended)
{
	struct ingest_refresh_wait w;
	ingest_refresh_wait_init(&w);
	ingest_refresh_wait_reset(&w);

	struct fake_refresh f = {&w, delay_ms, success};
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, NULL, fake_refresh_thread, &f), 0);

	uint64_t start = os_gettime_ns();
	*ended = ingest_refresh_wait_for(&w, timeout_ms);
	uint64_t elapsed_ms = (os_gettime_ns() - start) / 1000000;

	pthread_join(thread, NULL);
	ingest_refresh_wait_free(&w);
	return elapsed_ms;
}

static void successful_refresh_ends_wait(void **state)
{
	(void)state;
	bool ended;
	uint64_t ms = run_wait(true, 50, 5000, &ended);
	assert_true(ended);
	assert_true(ms >= 40);
	assert_true(ms < 1000);
}

/* A refresh whose download arrived but did not parse still ends the wait:
 * the old loop kept polling ingests_refreshed and waited the full timeout. */
static void failed_refresh_ends_wait(void **state)
{
	(void)state;
	bool ended;
	uint64_t ms = run_wait(false, 50, 5000, &ended);
	assert_true(ended);
	assert_true(ms < 1000);
}

static void refresh_that_never_ends_times_out(void **state)
{
	(void)state;
	struct ingest_refresh_wait w;
	ingest_refresh_wait_init(&w);
	ingest_refresh_wait_reset(&w);

	uint64_t start = os_gettime_ns();
	assert_false(ingest_refresh_wait_for(&w, 300));
	uint64_t ms = (os_gettime_ns() - start) / 1000000;
	assert_true(ms >= 290);
	assert_true(ms < 1000);

	ingest_refresh_wait_free(&w);
}

static void completed_wait_returns_at_once_until_reset(void **state)
{
	(void)state;
	struct ingest_refresh_wait w;
	ingest_refresh_wait_init(&w);
	ingest_refresh_wait_reset(&w);
	ingest_refresh_wait_complete(&w, false);

	uint64_t start = os_gettime_ns();
	assert_true(ingest_refresh_wait_for(&w, 5000));
	assert_true((os_gettime_ns() - start) / 1000000 < 50);

	ingest_refresh_wait_reset(&w);
	assert_false(ingest_refresh_wait_for(&w, 50));
	ingest_refresh_wait_free(&w);
}

/* ingests_loaded: a usable list exists (cache or download).
 * ingests_refreshed: this session's download succeeded (sets loaded too).
 * Only a caller with no list at all has to wait. */
static void needs_wait_only_without_any_list(void **state)
{
	(void)state;
	assert_true(ingest_refresh_needs_wait(false, false));
	assert_false(ingest_refresh_needs_wait(false, true));
	assert_false(ingest_refresh_needs_wait(true, false));
	assert_false(ingest_refresh_needs_wait(true, true));
}

/* os_event_init() failed, so there is no event (it stays NULL). Starting
 * and ending a refresh does nothing, and the wait ends at once: there is
 * nothing to wait on, and the caller goes on with the list it has. */
static void wait_without_event_ends_at_once(void **state)
{
	(void)state;
	struct ingest_refresh_wait w = {.done = NULL};

	ingest_refresh_wait_reset(&w);
	ingest_refresh_wait_complete(&w, true);

	uint64_t start = os_gettime_ns();
	assert_false(ingest_refresh_wait_for(&w, 5000));
	uint64_t elapsed_ms = (os_gettime_ns() - start) / 1000000;
	assert_true(elapsed_ms < 1000);

	ingest_refresh_wait_free(&w);
	assert_null(w.done);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(successful_refresh_ends_wait),
		cmocka_unit_test(failed_refresh_ends_wait),
		cmocka_unit_test(refresh_that_never_ends_times_out),
		cmocka_unit_test(completed_wait_returns_at_once_until_reset),
		cmocka_unit_test(needs_wait_only_without_any_list),
		cmocka_unit_test(wait_without_event_ends_at_once),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
