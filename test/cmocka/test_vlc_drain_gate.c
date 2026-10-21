#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <util/c99defs.h>
#include <util/platform.h>
#include "vlc-drain-gate.h"

/*
 * The gate that cuts the VLC source's audio waits short
 * (obs-rust/obs-studio#1011). A Stop must stay in force even when libVLC's
 * event thread reports the next playlist item opening right after it: that
 * ordering made Stop near a track boundary take up to 3 s.
 */

#define LONG_WAIT_MS 2000
/* far below LONG_WAIT_MS, generous for a loaded machine */
#define PROMPT_NS (500 * 1000000ULL)

static uint64_t timed_wait(struct vlc_drain_gate *g)
{
	uint64_t t0 = os_gettime_ns();
	vlc_drain_gate_wait(g, LONG_WAIT_MS);
	return os_gettime_ns() - t0;
}

static void test_stop_survives_the_next_item_opening(void **state)
{
	UNUSED_PARAMETER(state);
	struct vlc_drain_gate g = {0};
	assert_true(vlc_drain_gate_init(&g));

	vlc_drain_gate_play(&g);
	assert_true(vlc_drain_gate_opened(&g));
	assert_false(vlc_drain_gate_stopping(&g));
	/* Stop, then libVLC's Opening event for the next item arrives late:
	 * that item is not to play */
	vlc_drain_gate_stop(&g);
	assert_true(vlc_drain_gate_stopping(&g));
	assert_false(vlc_drain_gate_opened(&g));
	assert_true(timed_wait(&g) < PROMPT_NS);

	vlc_drain_gate_free(&g);
}

static void test_waits_work_again_after_play(void **state)
{
	UNUSED_PARAMETER(state);
	struct vlc_drain_gate g = {0};
	assert_true(vlc_drain_gate_init(&g));

	vlc_drain_gate_stop(&g);
	vlc_drain_gate_play(&g);
	assert_false(vlc_drain_gate_stopping(&g));
	assert_true(vlc_drain_gate_opened(&g));
	/* the new item's waits must not be cut short */
	uint64_t t0 = os_gettime_ns();
	vlc_drain_gate_wait(&g, 100);
	assert_true(os_gettime_ns() - t0 >= 90 * 1000000ULL);

	vlc_drain_gate_free(&g);
}

static void test_next_item_cuts_the_wait_then_rearms(void **state)
{
	UNUSED_PARAMETER(state);
	struct vlc_drain_gate g = {0};
	assert_true(vlc_drain_gate_init(&g));

	vlc_drain_gate_play(&g);
	vlc_drain_gate_opened(&g);
	vlc_drain_gate_abort(&g);
	assert_true(timed_wait(&g) < PROMPT_NS);
	/* the next item opens: its waits work again */
	vlc_drain_gate_opened(&g);
	uint64_t t0 = os_gettime_ns();
	vlc_drain_gate_wait(&g, 100);
	assert_true(os_gettime_ns() - t0 >= 90 * 1000000ULL);

	vlc_drain_gate_free(&g);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_stop_survives_the_next_item_opening),
		cmocka_unit_test(test_waits_work_again_after_play),
		cmocka_unit_test(test_next_item_cuts_the_wait_then_rearms),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
