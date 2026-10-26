#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-source-video-tick.h"

/*
 * Characterization of the per-frame decisions of obs_source_video_tick()
 * and async_tick() in libobs/obs-source.c, moved to
 * libobs/obs-source-video-tick.c before its Rust port:
 * - obs_source_video_tick_transition(), _async() and _media(): which steps
 *   run for the source's type and output flags;
 * - obs_source_video_tick_deferred_update(): whether a deferred update is
 *   due for the loaded defer_update_count;
 * - obs_source_video_tick_edge(): the show/hide and activate/deactivate
 *   edges of a loaded reference count against the flag it sets;
 * - obs_source_video_tick_calls_tick(): whether the plugin's video_tick
 *   callback is called;
 * - obs_source_video_tick_async_route(): what async_tick() does with the
 *   frames.
 * The last cases run a copy of obs_source_video_tick()'s order of steps on
 * a stand-in source, so the composition is pinned too: the show refs are
 * loaded before the show edge runs, the activate refs only after it, and the
 * filters are walked in reverse.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_visibility_transition,
 * test_preview_only_hide, test_studio_mode_swap, test_studio_mode_exit,
 * test_canvas_destroy_scene_refs and test_item_transition_empty_source
 * cover the callers through the real libobs.
 */

enum {
	NONE = OBS_SOURCE_VIDEO_TICK_EDGE_NONE,
	ON = OBS_SOURCE_VIDEO_TICK_EDGE_ON,
	OFF = OBS_SOURCE_VIDEO_TICK_EDGE_OFF,
	CLOSEST = OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST,
	DEINTERLACE = OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE,
};

/* the source types, then values outside the enum */
static const int TYPES[] = {OBS_SOURCE_TYPE_INPUT,
			    OBS_SOURCE_TYPE_FILTER,
			    OBS_SOURCE_TYPE_TRANSITION,
			    OBS_SOURCE_TYPE_SCENE,
			    -1,
			    4,
			    INT_MIN,
			    INT_MAX};
#define TYPE_COUNT (sizeof(TYPES) / sizeof(TYPES[0]))

/* loaded counts: 0, the usual ones, negative ones and the ends of long */
static const long COUNTS[] = {0, 1, 2, 3, 100, -1, -2, LONG_MIN, LONG_MIN + 1, LONG_MAX, LONG_MAX - 1};
#define COUNT_COUNT (sizeof(COUNTS) / sizeof(COUNTS[0]))

#define CHECK_STR(a, b)                                                                     \
	do {                                                                                \
		if (strcmp((a), (b)) != 0) {                                                \
			fprintf(stderr, "%s:%d: got \"%s\", expected \"%s\"\n", __FILE__, __LINE__, \
				(a), (b));                                                  \
			fail();                                                             \
		}                                                                           \
	} while (0)

static void enum_values(void **state)
{
	(void)state;

	assert_int_equal(NONE, 0);
	assert_int_equal(ON, 1);
	assert_int_equal(OFF, 2);
	assert_int_equal(CLOSEST, 0);
	assert_int_equal(DEINTERLACE, 1);

	/* the header's mirrors of obs-source.h */
	assert_int_equal(OBS_SOURCE_VIDEO_TICK_TYPE_TRANSITION, OBS_SOURCE_TYPE_TRANSITION);
	assert_int_equal(OBS_SOURCE_VIDEO_TICK_TYPE_TRANSITION, 2);
	assert_int_equal(OBS_SOURCE_VIDEO_TICK_ASYNC, OBS_SOURCE_ASYNC);
	assert_int_equal(OBS_SOURCE_VIDEO_TICK_ASYNC, 1u << 2);
	assert_int_equal(OBS_SOURCE_VIDEO_TICK_CONTROLLABLE_MEDIA, OBS_SOURCE_CONTROLLABLE_MEDIA);
	assert_int_equal(OBS_SOURCE_VIDEO_TICK_CONTROLLABLE_MEDIA, 1u << 13);
	assert_int_equal(sizeof(enum obs_source_video_tick_edge), sizeof(int));
	assert_int_equal(sizeof(enum obs_source_video_tick_async_route), sizeof(int));
}

static void transition_every_type(void **state)
{
	(void)state;

	for (size_t i = 0; i < TYPE_COUNT; i++)
		assert_int_equal(obs_source_video_tick_transition(TYPES[i]), TYPES[i] == OBS_SOURCE_TYPE_TRANSITION);

	/* every value near the transition type, so only 2 is one */
	for (int type = -64; type <= 64; type++)
		assert_int_equal(obs_source_video_tick_transition(type), type == 2);
}

static void async_every_bit(void **state)
{
	(void)state;

	assert_false(obs_source_video_tick_async(0));
	assert_true(obs_source_video_tick_async(UINT32_MAX));
	assert_false(obs_source_video_tick_async(~(uint32_t)OBS_SOURCE_ASYNC));
	assert_true(obs_source_video_tick_async(OBS_SOURCE_ASYNC_VIDEO));
	assert_true(obs_source_video_tick_async(OBS_SOURCE_ASYNC | OBS_SOURCE_CONTROLLABLE_MEDIA));
	assert_false(obs_source_video_tick_async(OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO | OBS_SOURCE_CONTROLLABLE_MEDIA));

	for (unsigned bit = 0; bit < 32; bit++) {
		uint32_t flag = (uint32_t)1 << bit;
		assert_int_equal(obs_source_video_tick_async(flag), bit == 2);
		assert_int_equal(obs_source_video_tick_async(~flag), bit != 2);
	}
}

static void media_every_bit(void **state)
{
	(void)state;

	assert_false(obs_source_video_tick_media(0));
	assert_true(obs_source_video_tick_media(UINT32_MAX));
	assert_false(obs_source_video_tick_media(~(uint32_t)OBS_SOURCE_CONTROLLABLE_MEDIA));
	assert_true(obs_source_video_tick_media(OBS_SOURCE_CONTROLLABLE_MEDIA | OBS_SOURCE_ASYNC_VIDEO));
	assert_false(obs_source_video_tick_media(OBS_SOURCE_ASYNC_VIDEO | OBS_SOURCE_AUDIO));

	for (unsigned bit = 0; bit < 32; bit++) {
		uint32_t flag = (uint32_t)1 << bit;
		assert_int_equal(obs_source_video_tick_media(flag), bit == 13);
		assert_int_equal(obs_source_video_tick_media(~flag), bit != 13);
	}
}

/* Each type with each combination of the two flags and of other bits: the
 * three steps are independent, the transition tick on the type alone, the
 * async tick and the media actions on their flag alone. Characterized, not
 * endorsed: the type is not checked against the flags, so a filter or a
 * scene flagged async gets the async tick, and a transition flagged
 * controllable media gets all three. */
static void steps_every_type_and_flags(void **state)
{
	(void)state;

	static const uint32_t OTHER[] = {0, OBS_SOURCE_VIDEO, OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE,
					 ~(uint32_t)(OBS_SOURCE_ASYNC | OBS_SOURCE_CONTROLLABLE_MEDIA)};

	for (size_t t = 0; t < TYPE_COUNT; t++) {
		for (unsigned combo = 0; combo < 4; combo++) {
			for (size_t o = 0; o < sizeof(OTHER) / sizeof(OTHER[0]); o++) {
				uint32_t flags = OTHER[o];
				if (combo & 1)
					flags |= OBS_SOURCE_ASYNC;
				if (combo & 2)
					flags |= OBS_SOURCE_CONTROLLABLE_MEDIA;

				assert_int_equal(obs_source_video_tick_transition(TYPES[t]), t == 2);
				assert_int_equal(obs_source_video_tick_async(flags), (combo & 1) != 0);
				assert_int_equal(obs_source_video_tick_media(flags), (combo & 2) != 0);
			}
		}
	}
}

/* Only a count above 0 is due. Characterized, not endorsed: a negative
 * count (obs_source_deferred_update() swaps the count it loaded for 0, so
 * none is expected) is not due, and neither is 0. */
static void deferred_update_counts(void **state)
{
	(void)state;

	for (size_t i = 0; i < COUNT_COUNT; i++)
		assert_int_equal(obs_source_video_tick_deferred_update(COUNTS[i]), COUNTS[i] > 0);

	for (long count = -100; count <= 100; count++)
		assert_int_equal(obs_source_video_tick_deferred_update(count), count > 0);
}

/* Both edges in both directions and no edge. */
static void edge_both_directions(void **state)
{
	(void)state;

	/* hidden, then shown: on */
	assert_int_equal(obs_source_video_tick_edge(1, false), ON);
	/* shown, then hidden: off */
	assert_int_equal(obs_source_video_tick_edge(0, true), OFF);
	/* no change: none, either way */
	assert_int_equal(obs_source_video_tick_edge(0, false), NONE);
	assert_int_equal(obs_source_video_tick_edge(1, true), NONE);

	/* any count but 0 is showing or active, so a count going from 1 to 2
	 * while the flag is set is no edge */
	assert_int_equal(obs_source_video_tick_edge(2, true), NONE);
	assert_int_equal(obs_source_video_tick_edge(2, false), ON);
}

/* Characterized, not endorsed: a negative count (one release too many) is
 * not 0, so it shows or activates the source and keeps it so. */
static void edge_every_count(void **state)
{
	(void)state;

	for (size_t i = 0; i < COUNT_COUNT; i++) {
		long refs = COUNTS[i];
		bool now = refs != 0;

		assert_int_equal(obs_source_video_tick_edge(refs, now), NONE);
		assert_int_equal(obs_source_video_tick_edge(refs, !now), now ? ON : OFF);
	}

	assert_int_equal(obs_source_video_tick_edge(-1, false), ON);
	assert_int_equal(obs_source_video_tick_edge(-1, true), NONE);
	assert_int_equal(obs_source_video_tick_edge(LONG_MIN, false), ON);
	assert_int_equal(obs_source_video_tick_edge(LONG_MAX, true), NONE);
}

/* A run of ticks from a hidden start: the caller stores the flag on each
 * edge, so only the moves between 0 and not 0 are edges. */
static void edge_sequence(void **state)
{
	(void)state;

	static const long refs[] = {0, 1, 2, 1, 0, 0, 3, 0, -1, 0};
	char trace[128] = "";
	bool flag = false;

	for (size_t i = 0; i < sizeof(refs) / sizeof(refs[0]); i++) {
		enum obs_source_video_tick_edge edge = obs_source_video_tick_edge(refs[i], flag);
		strcat(trace, edge == ON ? "on " : edge == OFF ? "off " : "- ");
		if (edge != OBS_SOURCE_VIDEO_TICK_EDGE_NONE)
			flag = edge == OBS_SOURCE_VIDEO_TICK_EDGE_ON;
	}

	CHECK_STR(trace, "- on - - off - on off on off ");
}

/* The callback is called only with data and a callback. */
static void calls_tick_truth_table(void **state)
{
	(void)state;

	assert_false(obs_source_video_tick_calls_tick(false, false));
	assert_false(obs_source_video_tick_calls_tick(false, true));
	assert_false(obs_source_video_tick_calls_tick(true, false));
	assert_true(obs_source_video_tick_calls_tick(true, true));
}

static void async_route_both(void **state)
{
	(void)state;

	assert_int_equal(obs_source_video_tick_async_route(false), CLOSEST);
	assert_int_equal(obs_source_video_tick_async_route(true), DEINTERLACE);
}

/* A stand-in source and a copy of obs_source_video_tick()'s order of steps
 * and of async_tick()'s, from libobs/obs-source.c, with each call written to
 * a trace. */
struct standin {
	int type;
	uint32_t flags;
	long defer;
	long show_refs;
	long activate_refs;
	bool showing;
	bool active;
	size_t filters;
	bool has_data;
	bool has_callback;
	bool deinterlacing;
	/* a show callback that adds an activate reference, as a scene's
	 * child can */
	bool show_activates;
	char trace[256];
};

static void put(struct standin *s, const char *what)
{
	strcat(s->trace, what);
	strcat(s->trace, " ");
}

static void put_filter(struct standin *s, const char *what, size_t i)
{
	char buf[64];
	snprintf(buf, sizeof(buf), "%.16s(f%u)", what, (unsigned)(i % 10));
	put(s, buf);
}

static void async_tick_copy(struct standin *s)
{
	put(s, "lock");
	if (obs_source_video_tick_async_route(s->deinterlacing) == OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE)
		put(s, "deinterlace");
	else
		put(s, "closest");
	if (obs_source_video_tick_async_route(s->deinterlacing) == OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE)
		put(s, "filter(prev)");
	put(s, "filter(cur)");
	put(s, "unlock");
}

static void tick_copy(struct standin *s)
{
	s->trace[0] = '\0';

	if (obs_source_video_tick_transition(s->type))
		put(s, "transition");
	if (obs_source_video_tick_async(s->flags))
		async_tick_copy(s);
	if (obs_source_video_tick_media(s->flags))
		put(s, "media");
	if (obs_source_video_tick_deferred_update(s->defer))
		put(s, "deferred");

	enum obs_source_video_tick_edge edge = obs_source_video_tick_edge(s->show_refs, s->showing);
	if (edge != OBS_SOURCE_VIDEO_TICK_EDGE_NONE) {
		bool now = edge == OBS_SOURCE_VIDEO_TICK_EDGE_ON;
		put(s, now ? "show" : "hide");
		if (now && s->show_activates)
			s->activate_refs++;
		for (size_t i = s->filters; i > 0; i--)
			put_filter(s, now ? "show" : "hide", i - 1);
		s->showing = now;
	}

	/* loaded after the show edge */
	edge = obs_source_video_tick_edge(s->activate_refs, s->active);
	if (edge != OBS_SOURCE_VIDEO_TICK_EDGE_NONE) {
		bool now = edge == OBS_SOURCE_VIDEO_TICK_EDGE_ON;
		put(s, now ? "activate" : "deactivate");
		for (size_t i = s->filters; i > 0; i--)
			put_filter(s, now ? "activate" : "deactivate", i - 1);
		s->active = now;
	}

	if (obs_source_video_tick_calls_tick(s->has_data, s->has_callback))
		put(s, "video_tick");
}

static void tick_order(void **state)
{
	(void)state;

	/* a plain input with data and a callback, shown and activated on its
	 * first tick */
	struct standin s = {.type = OBS_SOURCE_TYPE_INPUT,
			    .show_refs = 1,
			    .activate_refs = 1,
			    .has_data = true,
			    .has_callback = true};
	tick_copy(&s);
	CHECK_STR(s.trace, "show activate video_tick ");
	assert_true(s.showing);
	assert_true(s.active);
	tick_copy(&s);
	CHECK_STR(s.trace, "video_tick ");

	/* every step, the filters walked in reverse, then hidden and
	 * deactivated */
	struct standin all = {.type = OBS_SOURCE_TYPE_TRANSITION,
			      .flags = OBS_SOURCE_ASYNC_VIDEO | OBS_SOURCE_CONTROLLABLE_MEDIA,
			      .defer = 1,
			      .show_refs = 2,
			      .activate_refs = 1,
			      .filters = 3,
			      .has_data = true,
			      .has_callback = true,
			      .deinterlacing = true};
	tick_copy(&all);
	CHECK_STR(all.trace, "transition lock deinterlace filter(prev) filter(cur) unlock media deferred show "
			     "show(f2) show(f1) show(f0) activate activate(f2) activate(f1) activate(f0) video_tick ");
	all.show_refs = 0;
	all.activate_refs = 0;
	all.defer = 0;
	all.deinterlacing = false;
	all.has_callback = false;
	tick_copy(&all);
	CHECK_STR(all.trace, "transition lock closest filter(cur) unlock media hide hide(f2) hide(f1) hide(f0) "
			     "deactivate deactivate(f2) deactivate(f1) deactivate(f0) ");
	assert_false(all.showing);
	assert_false(all.active);

	/* the activate refs are loaded after the show edge: a show callback
	 * that adds one activates the source in the same tick */
	struct standin child = {.type = OBS_SOURCE_TYPE_SCENE, .show_refs = 1, .show_activates = true};
	tick_copy(&child);
	CHECK_STR(child.trace, "show activate ");

	/* no data: no callback, the edges still run */
	struct standin no_data = {.type = OBS_SOURCE_TYPE_FILTER, .show_refs = 1, .has_callback = true};
	tick_copy(&no_data);
	CHECK_STR(no_data.trace, "show ");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enum_values),
		cmocka_unit_test(transition_every_type),
		cmocka_unit_test(async_every_bit),
		cmocka_unit_test(media_every_bit),
		cmocka_unit_test(steps_every_type_and_flags),
		cmocka_unit_test(deferred_update_counts),
		cmocka_unit_test(edge_both_directions),
		cmocka_unit_test(edge_every_count),
		cmocka_unit_test(edge_sequence),
		cmocka_unit_test(calls_tick_truth_table),
		cmocka_unit_test(async_route_both),
		cmocka_unit_test(tick_order),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
