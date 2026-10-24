/* linux-capture: the XComposite window capture keeps whether its window is
 * viewable from X events instead of a round trip per frame (#1356).
 *
 * xcompcap_video_tick called xcomp_window_exists (a blocking
 * xcb_get_window_attributes request and reply) once or twice per frame under
 * obs_enter_graphics, so a busy or remote X server stalled rendering. This
 * drives the helpers the tick uses with simulated frames, time and a fake
 * event feed, and counts the round trips. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include "plugins/linux-capture/xcomposite-presence.h"

#define TICK (1.0 / 60.0)

struct sim {
	struct xcomp_presence st;
	bool server_viewable; /* what the X server would answer */
	int round_trips;
};

/* One tick, the way xcompcap_video_tick uses the helpers: returns whether
 * the tick treats the window as viewable. */
static bool tick(struct sim *s)
{
	if (xcomp_presence_check_due(&s->st, TICK)) {
		s->round_trips++;
		xcomp_presence_set(&s->st, s->server_viewable);
	}
	return s->st.viewable;
}

static void run(struct sim *s, double seconds)
{
	int n = (int)(seconds / TICK + 0.5);
	for (int i = 0; i < n; i++)
		tick(s);
}

static void init(struct sim *s)
{
	*s = (struct sim){0};
	s->server_viewable = true;
	xcomp_presence_invalidate(&s->st);
}

static void test_stable_window_rarely_asks_server(void **state)
{
	(void)state;
	struct sim s;
	init(&s);
	run(&s, 10.0);
	/* 600 frames: once per frame was 600 round trips (1200 before #1366). */
	assert_int_in_range(s.round_trips, 1, 11);
	assert_true(s.st.viewable);
}

static void test_events_flip_state_without_round_trip(void **state)
{
	(void)state;
	struct sim s;
	init(&s);
	tick(&s);
	int before = s.round_trips;

	s.server_viewable = false;
	xcomp_presence_event(&s.st, XCOMP_PRESENCE_UNMAPPED);
	assert_false(s.st.viewable);
	s.server_viewable = true;
	xcomp_presence_event(&s.st, XCOMP_PRESENCE_MAPPED);
	assert_true(s.st.viewable);
	s.server_viewable = false;
	xcomp_presence_event(&s.st, XCOMP_PRESENCE_DESTROYED);
	assert_false(s.st.viewable);
	xcomp_presence_event(&s.st, XCOMP_PRESENCE_OTHER);
	assert_false(s.st.viewable);
	assert_int_equal(s.round_trips, before);

	/* The next frame reads the cached state, no round trip. */
	assert_false(tick(&s));
	assert_int_equal(s.round_trips, before);
}

static void test_missed_event_caught_within_a_second(void **state)
{
	(void)state;
	struct sim s;
	init(&s);
	run(&s, 3.0);

	/* The window unmaps but the event is lost. */
	s.server_viewable = false;
	int frames = 0;
	while (tick(&s) && frames < 600)
		frames++;
	assert_true(frames <= 61);
}

static void test_invalidate_asks_next_tick(void **state)
{
	(void)state;
	struct sim s;
	init(&s);
	run(&s, 2.5); /* half-way between two safety-net checks */
	int before = s.round_trips;
	tick(&s);
	assert_int_equal(s.round_trips, before);
	xcomp_presence_invalidate(&s.st);
	s.server_viewable = false;
	assert_false(tick(&s));
	assert_int_equal(s.round_trips, before + 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_stable_window_rarely_asks_server),
		cmocka_unit_test(test_events_flip_state_without_round_trip),
		cmocka_unit_test(test_missed_event_caught_within_a_second),
		cmocka_unit_test(test_invalidate_asks_next_tick),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
