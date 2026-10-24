/* linux-capture: the XComposite window capture backs off its window search
 * while the window stays unmapped (#1357).
 *
 * A window that is minimized or on another workspace is not viewable, so the
 * capture counts it as lost. The tick searched for it every 2 s forever (the
 * client list plus name and class round trips per window, under
 * obs_enter_graphics) and rebuilt the pixmap and watcher each time, even when
 * the search found the same window. This drives the decision helpers the
 * tick uses with simulated time and window state. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include "plugins/linux-capture/xcomposite-search.h"

#define TICK (1.0 / 60.0)
#define WIN 0x2a00007u

struct sim {
	struct xcomp_search st;
	uint32_t win;
	bool viewable;
	bool have_tex;
	int searches;
	int rebuilds;
};

/* One tick, the way xcompcap_video_tick uses the helpers. */
static void tick(struct sim *s, bool forced)
{
	bool lost = !s->viewable || !s->have_tex;
	if (!xcomp_search_due(&s->st, TICK, lost, forced))
		return;
	s->searches++;
	uint32_t found = WIN;
	if (xcomp_search_needs_rebuild(s->win, found, s->viewable, forced)) {
		s->rebuilds++;
		s->win = found;
		s->have_tex = s->viewable;
	}
	xcomp_search_done(&s->st, s->have_tex, forced);
}

static void run(struct sim *s, double seconds)
{
	int n = (int)(seconds / TICK + 0.5);
	for (int i = 0; i < n; i++)
		tick(s, false);
}

static void init(struct sim *s)
{
	*s = (struct sim){0};
	xcomp_search_reset(&s->st);
	s->win = WIN;
}

static void test_unmapped_window_backs_off_and_keeps_pixmap(void **state)
{
	(void)state;
	struct sim s;
	init(&s);

	run(&s, 60.0);

	/* Every 2 s would be 29 searches; 2+4+8+16+30 s is 5 within 60 s. */
	assert_int_in_range(s.searches, 3, 6);
	assert_int_equal(s.rebuilds, 0);
	assert_true(s.st.interval <= XCOMP_SEARCH_INTERVAL_MAX);
}

static void test_map_event_resets_interval(void **state)
{
	(void)state;
	struct sim s;
	init(&s);
	run(&s, 60.0);
	assert_true(s.st.interval >= 16.0);

	/* MapNotify: the watcher sets window_changed, the next tick searches. */
	s.viewable = true;
	int before = s.searches;
	tick(&s, true);
	assert_int_equal(s.searches, before + 1);
	assert_int_equal(s.rebuilds, 1);
	assert_true(s.have_tex);

	/* Captured again: no search while the window stays viewable. */
	before = s.searches;
	run(&s, 10.0);
	assert_int_equal(s.searches, before);

	/* Lost again: the interval starts at 2 s, not where it left off. */
	s.viewable = false;
	s.have_tex = false;
	before = s.searches;
	run(&s, 2.5);
	assert_int_equal(s.searches, before + 1);
}

static void test_window_changed_forces_search(void **state)
{
	(void)state;
	struct sim s;
	init(&s);
	run(&s, 40.0);
	int before = s.searches;
	tick(&s, true);
	assert_int_equal(s.searches, before + 1);
	/* A forced search rebuilds even for the same window (it may have resized). */
	assert_int_equal(s.rebuilds, 1);
	assert_true(s.st.interval == XCOMP_SEARCH_INTERVAL_MIN);
}

static void test_new_window_is_rebuilt(void **state)
{
	(void)state;
	struct xcomp_search st;
	xcomp_search_reset(&st);
	assert_false(xcomp_search_due(&st, 1.0, true, false));
	assert_true(xcomp_search_due(&st, 1.5, true, false));
	assert_false(xcomp_search_due(&st, 100.0, false, false));
	assert_true(xcomp_search_needs_rebuild(WIN, WIN + 1, false, false));
	assert_true(xcomp_search_needs_rebuild(WIN, WIN, true, false));
	assert_false(xcomp_search_needs_rebuild(WIN, WIN, false, false));
	assert_false(xcomp_search_needs_rebuild(0, 0, false, false));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_unmapped_window_backs_off_and_keeps_pixmap),
		cmocka_unit_test(test_map_event_resets_interval),
		cmocka_unit_test(test_window_changed_forces_search),
		cmocka_unit_test(test_new_window_is_rebuilt),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
