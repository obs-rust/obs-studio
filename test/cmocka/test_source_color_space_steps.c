#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/graphics.h"
#include "obs-source-color-space.h"

/*
 * Characterization of the colour space query routing of
 * obs_source_get_color_space() in libobs/obs-source.c, moved to
 * libobs/obs-source-color-space.c before its Rust port:
 * - obs_source_color_space_route(): a non-filter source with no video asks
 *   its filter parent; a filter with no video asks its target (#1493); a
 *   source with no data, or a disabled one, asks its target; an async
 *   source picks; any other asks its callback;
 * - obs_source_color_space_pick(): the first preferred space equal to the
 *   async frame's space, else the last preferred space, else the frame's
 *   space.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_filter_color_space covers the caller
 * (data_valid(), the recursion, convert_video_space() and the callback)
 * through the real libobs, with the HDR Tone Mapping cases of #1493.
 */

/* enum obs_source_type and the output flags (obs-source.h, which needs the
 * whole of obs.h); obs-source.c checks the header's against them */
#define TYPE_INPUT 0
#define TYPE_FILTER OBS_SOURCE_COLOR_SPACE_TYPE_FILTER
#define TYPE_TRANSITION 2
#define TYPE_SCENE 3
#define VIDEO OBS_SOURCE_COLOR_SPACE_VIDEO
#define ASYNC OBS_SOURCE_COLOR_SPACE_ASYNC
#define AUDIO (1u << 1)        /* OBS_SOURCE_AUDIO */
#define ASYNC_VIDEO (ASYNC | VIDEO) /* OBS_SOURCE_ASYNC_VIDEO */

#define PARENT OBS_SOURCE_COLOR_SPACE_ASK_PARENT
#define TARGET OBS_SOURCE_COLOR_SPACE_ASK_TARGET
#define PICK OBS_SOURCE_COLOR_SPACE_ASYNC_PICK
#define CALLBACK OBS_SOURCE_COLOR_SPACE_CALLBACK

static const char *route_name(enum obs_source_color_space_ask route)
{
	switch (route) {
	case PARENT:
		return "ask parent";
	case TARGET:
		return "ask target";
	case PICK:
		return "async pick";
	case CALLBACK:
		return "callback";
	}
	return "(out of range)";
}

static struct obs_source_color_space_node node_of(int type, uint32_t flags, bool data, bool enabled, bool parent,
						  bool target)
{
	struct obs_source_color_space_node node = {
		.type = type,
		.output_flags = flags,
		.has_data = data,
		.enabled = enabled,
		.has_filter_parent = parent,
		.has_filter_target = target,
	};
	return node;
}

static void check_route_line(struct obs_source_color_space_node node, enum obs_source_color_space_ask expected,
			     int line)
{
	const enum obs_source_color_space_ask actual = obs_source_color_space_route(node);
	if (actual != expected) {
		fprintf(stderr,
			"line %d: type %d, flags 0x%x, data %d, enabled %d, parent %d, target %d: %s (%d), expected %s\n",
			line, node.type, node.output_flags, node.has_data, node.enabled, node.has_filter_parent,
			node.has_filter_target, route_name(actual), (int)actual, route_name(expected));
		fail();
	}
}

#define check_route(node, expected) check_route_line(node, expected, __LINE__)

static void check_pick_line(int video_space, size_t count, const int *preferred, int expected, int line)
{
	const int actual = obs_source_color_space_pick(video_space, count, preferred);
	if (actual != expected) {
		fprintf(stderr, "line %d: pick of %d from %zu preferred spaces is %d, expected %d\n", line, video_space,
			count, actual, expected);
		fail();
	}
}

#define check_pick(video_space, preferred, expected) \
	check_pick_line(video_space, sizeof(preferred) / sizeof((preferred)[0]), preferred, expected, __LINE__)

/* Every combination of the inputs: a filter or not, the VIDEO and ASYNC
 * flags, data, enabled, a filter parent and a filter target. The six fields
 * give seven inputs, as output_flags carries two. */
struct route_row {
	bool filter, video, async, data, enabled, parent, target;
	enum obs_source_color_space_ask route;
};

/* filter, video, async, data, enabled, parent, target, route */
static const struct route_row route_rows[128] = {
	{0, 0, 0, 0, 0, 0, 0, CALLBACK}, {0, 0, 0, 0, 0, 0, 1, TARGET},   {0, 0, 0, 0, 0, 1, 0, PARENT},
	{0, 0, 0, 0, 0, 1, 1, PARENT},   {0, 0, 0, 0, 1, 0, 0, CALLBACK}, {0, 0, 0, 0, 1, 0, 1, TARGET},
	{0, 0, 0, 0, 1, 1, 0, PARENT},   {0, 0, 0, 0, 1, 1, 1, PARENT},   {0, 0, 0, 1, 0, 0, 0, CALLBACK},
	{0, 0, 0, 1, 0, 0, 1, TARGET},   {0, 0, 0, 1, 0, 1, 0, PARENT},   {0, 0, 0, 1, 0, 1, 1, PARENT},
	{0, 0, 0, 1, 1, 0, 0, CALLBACK}, {0, 0, 0, 1, 1, 0, 1, CALLBACK}, {0, 0, 0, 1, 1, 1, 0, PARENT},
	{0, 0, 0, 1, 1, 1, 1, PARENT},   {0, 0, 1, 0, 0, 0, 0, PICK},     {0, 0, 1, 0, 0, 0, 1, TARGET},
	{0, 0, 1, 0, 0, 1, 0, PARENT},   {0, 0, 1, 0, 0, 1, 1, PARENT},   {0, 0, 1, 0, 1, 0, 0, PICK},
	{0, 0, 1, 0, 1, 0, 1, TARGET},   {0, 0, 1, 0, 1, 1, 0, PARENT},   {0, 0, 1, 0, 1, 1, 1, PARENT},
	{0, 0, 1, 1, 0, 0, 0, PICK},     {0, 0, 1, 1, 0, 0, 1, TARGET},   {0, 0, 1, 1, 0, 1, 0, PARENT},
	{0, 0, 1, 1, 0, 1, 1, PARENT},   {0, 0, 1, 1, 1, 0, 0, PICK},     {0, 0, 1, 1, 1, 0, 1, PICK},
	{0, 0, 1, 1, 1, 1, 0, PARENT},   {0, 0, 1, 1, 1, 1, 1, PARENT},   {0, 1, 0, 0, 0, 0, 0, CALLBACK},
	{0, 1, 0, 0, 0, 0, 1, TARGET},   {0, 1, 0, 0, 0, 1, 0, CALLBACK}, {0, 1, 0, 0, 0, 1, 1, TARGET},
	{0, 1, 0, 0, 1, 0, 0, CALLBACK}, {0, 1, 0, 0, 1, 0, 1, TARGET},   {0, 1, 0, 0, 1, 1, 0, CALLBACK},
	{0, 1, 0, 0, 1, 1, 1, TARGET},   {0, 1, 0, 1, 0, 0, 0, CALLBACK}, {0, 1, 0, 1, 0, 0, 1, TARGET},
	{0, 1, 0, 1, 0, 1, 0, CALLBACK}, {0, 1, 0, 1, 0, 1, 1, TARGET},   {0, 1, 0, 1, 1, 0, 0, CALLBACK},
	{0, 1, 0, 1, 1, 0, 1, CALLBACK}, {0, 1, 0, 1, 1, 1, 0, CALLBACK}, {0, 1, 0, 1, 1, 1, 1, CALLBACK},
	{0, 1, 1, 0, 0, 0, 0, PICK},     {0, 1, 1, 0, 0, 0, 1, TARGET},   {0, 1, 1, 0, 0, 1, 0, PICK},
	{0, 1, 1, 0, 0, 1, 1, TARGET},   {0, 1, 1, 0, 1, 0, 0, PICK},     {0, 1, 1, 0, 1, 0, 1, TARGET},
	{0, 1, 1, 0, 1, 1, 0, PICK},     {0, 1, 1, 0, 1, 1, 1, TARGET},   {0, 1, 1, 1, 0, 0, 0, PICK},
	{0, 1, 1, 1, 0, 0, 1, TARGET},   {0, 1, 1, 1, 0, 1, 0, PICK},     {0, 1, 1, 1, 0, 1, 1, TARGET},
	{0, 1, 1, 1, 1, 0, 0, PICK},     {0, 1, 1, 1, 1, 0, 1, PICK},     {0, 1, 1, 1, 1, 1, 0, PICK},
	{0, 1, 1, 1, 1, 1, 1, PICK},     {1, 0, 0, 0, 0, 0, 0, CALLBACK}, {1, 0, 0, 0, 0, 0, 1, TARGET},
	{1, 0, 0, 0, 0, 1, 0, CALLBACK}, {1, 0, 0, 0, 0, 1, 1, TARGET},   {1, 0, 0, 0, 1, 0, 0, CALLBACK},
	{1, 0, 0, 0, 1, 0, 1, TARGET},   {1, 0, 0, 0, 1, 1, 0, CALLBACK}, {1, 0, 0, 0, 1, 1, 1, TARGET},
	{1, 0, 0, 1, 0, 0, 0, CALLBACK}, {1, 0, 0, 1, 0, 0, 1, TARGET},   {1, 0, 0, 1, 0, 1, 0, CALLBACK},
	{1, 0, 0, 1, 0, 1, 1, TARGET},   {1, 0, 0, 1, 1, 0, 0, CALLBACK}, {1, 0, 0, 1, 1, 0, 1, TARGET},
	{1, 0, 0, 1, 1, 1, 0, CALLBACK}, {1, 0, 0, 1, 1, 1, 1, TARGET},   {1, 0, 1, 0, 0, 0, 0, PICK},
	{1, 0, 1, 0, 0, 0, 1, TARGET},   {1, 0, 1, 0, 0, 1, 0, PICK},     {1, 0, 1, 0, 0, 1, 1, TARGET},
	{1, 0, 1, 0, 1, 0, 0, PICK},     {1, 0, 1, 0, 1, 0, 1, TARGET},   {1, 0, 1, 0, 1, 1, 0, PICK},
	{1, 0, 1, 0, 1, 1, 1, TARGET},   {1, 0, 1, 1, 0, 0, 0, PICK},     {1, 0, 1, 1, 0, 0, 1, TARGET},
	{1, 0, 1, 1, 0, 1, 0, PICK},     {1, 0, 1, 1, 0, 1, 1, TARGET},   {1, 0, 1, 1, 1, 0, 0, PICK},
	{1, 0, 1, 1, 1, 0, 1, TARGET},   {1, 0, 1, 1, 1, 1, 0, PICK},     {1, 0, 1, 1, 1, 1, 1, TARGET},
	{1, 1, 0, 0, 0, 0, 0, CALLBACK}, {1, 1, 0, 0, 0, 0, 1, TARGET},   {1, 1, 0, 0, 0, 1, 0, CALLBACK},
	{1, 1, 0, 0, 0, 1, 1, TARGET},   {1, 1, 0, 0, 1, 0, 0, CALLBACK}, {1, 1, 0, 0, 1, 0, 1, TARGET},
	{1, 1, 0, 0, 1, 1, 0, CALLBACK}, {1, 1, 0, 0, 1, 1, 1, TARGET},   {1, 1, 0, 1, 0, 0, 0, CALLBACK},
	{1, 1, 0, 1, 0, 0, 1, TARGET},   {1, 1, 0, 1, 0, 1, 0, CALLBACK}, {1, 1, 0, 1, 0, 1, 1, TARGET},
	{1, 1, 0, 1, 1, 0, 0, CALLBACK}, {1, 1, 0, 1, 1, 0, 1, CALLBACK}, {1, 1, 0, 1, 1, 1, 0, CALLBACK},
	{1, 1, 0, 1, 1, 1, 1, CALLBACK}, {1, 1, 1, 0, 0, 0, 0, PICK},     {1, 1, 1, 0, 0, 0, 1, TARGET},
	{1, 1, 1, 0, 0, 1, 0, PICK},     {1, 1, 1, 0, 0, 1, 1, TARGET},   {1, 1, 1, 0, 1, 0, 0, PICK},
	{1, 1, 1, 0, 1, 0, 1, TARGET},   {1, 1, 1, 0, 1, 1, 0, PICK},     {1, 1, 1, 0, 1, 1, 1, TARGET},
	{1, 1, 1, 1, 0, 0, 0, PICK},     {1, 1, 1, 1, 0, 0, 1, TARGET},   {1, 1, 1, 1, 0, 1, 0, PICK},
	{1, 1, 1, 1, 0, 1, 1, TARGET},   {1, 1, 1, 1, 1, 0, 0, PICK},     {1, 1, 1, 1, 1, 0, 1, PICK},
	{1, 1, 1, 1, 1, 1, 0, PICK},     {1, 1, 1, 1, 1, 1, 1, PICK},
};

static uint32_t flags_of(const struct route_row *row)
{
	return (row->video ? VIDEO : 0) | (row->async ? ASYNC : 0);
}

static void route_every_combination(void **state)
{
	(void)state;

	for (size_t i = 0; i < sizeof(route_rows) / sizeof(route_rows[0]); ++i) {
		const struct route_row *row = &route_rows[i];
		/* the rows count up, a binary number with the filter bit first */
		const size_t bits = (size_t)row->filter << 6 | (size_t)row->video << 5 | (size_t)row->async << 4 |
				    (size_t)row->data << 3 | (size_t)row->enabled << 2 | (size_t)row->parent << 1 |
				    (size_t)row->target;
		if (bits != i) {
			fprintf(stderr, "row %zu is out of order\n", i);
			fail();
		}
		check_route_line(node_of(row->filter ? TYPE_FILTER : TYPE_INPUT, flags_of(row), row->data, row->enabled,
					 row->parent, row->target),
				 row->route, (int)i);
	}
}

/* Only OBS_SOURCE_TYPE_FILTER counts as a filter: transitions, scenes and
 * values outside the enum route as inputs. */
static void route_other_types_as_inputs(void **state)
{
	(void)state;
	const int types[] = {TYPE_TRANSITION, TYPE_SCENE, -1, 4, INT_MIN, INT_MAX};

	for (size_t t = 0; t < sizeof(types) / sizeof(types[0]); ++t) {
		for (size_t i = 0; i < 64; ++i) {
			const struct route_row *row = &route_rows[i];
			check_route_line(node_of(types[t], flags_of(row), row->data, row->enabled, row->parent,
						 row->target),
					 row->route, (int)i);
		}
	}
}

/* Only the VIDEO and ASYNC bits count: every other flag set changes no
 * route. */
static void route_other_flags_ignored(void **state)
{
	(void)state;
	const uint32_t others = ~(VIDEO | ASYNC);

	for (size_t i = 0; i < 128; ++i) {
		const struct route_row *row = &route_rows[i];
		check_route_line(node_of(row->filter ? TYPE_FILTER : TYPE_INPUT, flags_of(row) | others, row->data,
					 row->enabled, row->parent, row->target),
				 row->route, (int)i);
		check_route_line(node_of(row->filter ? TYPE_FILTER : TYPE_INPUT, flags_of(row) | AUDIO, row->data,
					 row->enabled, row->parent, row->target),
				 row->route, (int)i);
	}
}

/* The chains of test_filter_color_space and #1493: an HDR async source
 * (ASYNC_VIDEO) under filters. */
static void route_filter_chains(void **state)
{
	(void)state;

	/* the HDR source itself picks */
	check_route(node_of(TYPE_INPUT, ASYNC_VIDEO | AUDIO, true, true, false, false), PICK);
	/* an audio filter above it asks its target (#1493: it used to answer
	 * GS_CS_SRGB from its own missing callback, so HDR Tone Mapping saw
	 * SDR) */
	check_route(node_of(TYPE_FILTER, AUDIO, true, true, false, true), TARGET);
	/* disabled, it still asks its target */
	check_route(node_of(TYPE_FILTER, AUDIO, true, false, false, true), TARGET);
	/* a video filter asks its own callback (GS_CS_SRGB without one) */
	check_route(node_of(TYPE_FILTER, VIDEO, true, true, false, true), CALLBACK);
	/* disabled, it asks its target */
	check_route(node_of(TYPE_FILTER, VIDEO, true, false, false, true), TARGET);
	/* a video filter with no data asks its target */
	check_route(node_of(TYPE_FILTER, VIDEO, false, true, false, true), TARGET);
	/* an audio filter with no target (detached) falls through to its
	 * callback: GS_CS_SRGB in the caller */
	check_route(node_of(TYPE_FILTER, AUDIO, true, true, false, false), CALLBACK);
	/* an audio-only input with a filter parent asks the parent; the parent
	 * comes before the target */
	check_route(node_of(TYPE_INPUT, AUDIO, true, true, true, false), PARENT);
	check_route(node_of(TYPE_INPUT, AUDIO, false, false, true, true), PARENT);
	/* an audio-only input with neither asks its callback */
	check_route(node_of(TYPE_INPUT, AUDIO, true, true, false, false), CALLBACK);
	/* a disabled video input asks its target, or its callback without one */
	check_route(node_of(TYPE_INPUT, VIDEO, true, false, false, true), TARGET);
	check_route(node_of(TYPE_INPUT, VIDEO, true, false, false, false), CALLBACK);
	/* a scene asks its callback */
	check_route(node_of(TYPE_SCENE, VIDEO, true, true, false, false), CALLBACK);
}

/* Characterized, not endorsed: a filter flagged async picks from its own
 * async frame's space like an async input, unless the earlier rules send it
 * to its target. Filters have no async frames, so in libobs that is the space
 * of async_format and async_trc as they are (0, 0: VIDEO_FORMAT_NONE gives
 * GS_CS_SRGB). */
static void route_filter_flagged_async(void **state)
{
	(void)state;

	check_route(node_of(TYPE_FILTER, ASYNC_VIDEO, true, true, false, true), PICK);
	check_route(node_of(TYPE_FILTER, ASYNC_VIDEO, true, true, false, false), PICK);
	/* ASYNC without VIDEO: #1493's rule comes first */
	check_route(node_of(TYPE_FILTER, ASYNC, true, true, false, true), TARGET);
	check_route(node_of(TYPE_FILTER, ASYNC, true, true, false, false), PICK);
	/* disabled, it asks its target */
	check_route(node_of(TYPE_FILTER, ASYNC_VIDEO, true, false, false, true), TARGET);
	/* disabled with no target, it still picks */
	check_route(node_of(TYPE_FILTER, ASYNC_VIDEO, true, false, false, false), PICK);
	/* a filter parent does not matter to a filter */
	check_route(node_of(TYPE_FILTER, ASYNC, true, true, true, false), PICK);
}

/* Characterized, not endorsed: with no data and nowhere to go, the route is
 * the callback (or the pick). obs_source_get_color_space() checks
 * data_valid() first, so libobs never gets here without data; the caller
 * asserts. */
static void route_no_data_nowhere_to_go(void **state)
{
	(void)state;

	check_route(node_of(TYPE_INPUT, VIDEO, false, true, false, false), CALLBACK);
	check_route(node_of(TYPE_FILTER, VIDEO, false, false, false, false), CALLBACK);
	check_route(node_of(TYPE_INPUT, ASYNC_VIDEO, false, false, false, false), PICK);
}

static void pick_empty_list(void **state)
{
	(void)state;

	/* obs_source_get_color_space(source, 0, NULL) */
	check_pick_line(GS_CS_SRGB, 0, NULL, GS_CS_SRGB, __LINE__);
	check_pick_line(GS_CS_SRGB_16F, 0, NULL, GS_CS_SRGB_16F, __LINE__);
	check_pick_line(GS_CS_709_EXTENDED, 0, NULL, GS_CS_709_EXTENDED, __LINE__);
	check_pick_line(GS_CS_709_SCRGB, 0, NULL, GS_CS_709_SCRGB, __LINE__);

	/* a count of 0 reads nothing, whatever the pointer */
	const int one[] = {GS_CS_SRGB};
	check_pick_line(GS_CS_709_EXTENDED, 0, one, GS_CS_709_EXTENDED, __LINE__);
}

static void pick_match_first(void **state)
{
	(void)state;

	const int hdr_first[] = {GS_CS_709_EXTENDED, GS_CS_SRGB, GS_CS_SRGB_16F};
	check_pick(GS_CS_709_EXTENDED, hdr_first, GS_CS_709_EXTENDED);

	const int sdr_first[] = {GS_CS_SRGB, GS_CS_709_EXTENDED};
	check_pick(GS_CS_SRGB, sdr_first, GS_CS_SRGB);

	/* a list of one */
	const int only[] = {GS_CS_SRGB_16F};
	check_pick(GS_CS_SRGB_16F, only, GS_CS_SRGB_16F);
}

static void pick_match_last(void **state)
{
	(void)state;

	/* the preferred spaces of the HDR Tone Mapping filter (#1493) */
	const int tonemap[] = {GS_CS_SRGB, GS_CS_SRGB_16F, GS_CS_709_EXTENDED};
	check_pick(GS_CS_709_EXTENDED, tonemap, GS_CS_709_EXTENDED);
	check_pick(GS_CS_SRGB_16F, tonemap, GS_CS_SRGB_16F);

	const int two[] = {GS_CS_SRGB, GS_CS_709_SCRGB};
	check_pick(GS_CS_709_SCRGB, two, GS_CS_709_SCRGB);
}

/* The loop stops at the first match: a later entry does not replace it. */
static void pick_match_in_the_middle(void **state)
{
	(void)state;

	const int middle[] = {GS_CS_SRGB, GS_CS_709_EXTENDED, GS_CS_709_SCRGB};
	check_pick(GS_CS_709_EXTENDED, middle, GS_CS_709_EXTENDED);

	const int twice[] = {GS_CS_SRGB_16F, GS_CS_709_EXTENDED, GS_CS_SRGB, GS_CS_709_EXTENDED};
	check_pick(GS_CS_709_EXTENDED, twice, GS_CS_709_EXTENDED);
	check_pick(GS_CS_SRGB, twice, GS_CS_SRGB);
}

/* No match: the last preferred space, not the frame's and not the first. */
static void pick_no_match(void **state)
{
	(void)state;

	const int sdr[] = {GS_CS_SRGB, GS_CS_SRGB_16F};
	check_pick(GS_CS_709_EXTENDED, sdr, GS_CS_SRGB_16F);

	const int one[] = {GS_CS_SRGB};
	check_pick(GS_CS_709_EXTENDED, one, GS_CS_SRGB);

	const int all_others[] = {GS_CS_709_SCRGB, GS_CS_709_EXTENDED, GS_CS_SRGB_16F};
	check_pick(GS_CS_SRGB, all_others, GS_CS_SRGB_16F);

	const int repeated[] = {GS_CS_709_SCRGB, GS_CS_709_SCRGB};
	check_pick(GS_CS_SRGB, repeated, GS_CS_709_SCRGB);
}

/* Characterized, not endorsed: a value outside enum gs_color_space in the
 * list is compared and returned as it is, never checked. */
static void pick_out_of_range_passed_through(void **state)
{
	(void)state;

	const int last_out[] = {GS_CS_SRGB_16F, 99};
	check_pick(GS_CS_709_EXTENDED, last_out, 99);

	const int negative[] = {-1};
	check_pick(GS_CS_SRGB, negative, -1);

	const int extremes[] = {INT_MIN, GS_CS_SRGB_16F, INT_MAX};
	check_pick(GS_CS_709_EXTENDED, extremes, INT_MAX);
	check_pick(GS_CS_SRGB_16F, extremes, GS_CS_SRGB_16F);

	/* one past the enum, as a frame's space: matched like any other */
	const int past_end[] = {GS_CS_SRGB, GS_CS_709_SCRGB + 1, GS_CS_709_EXTENDED};
	check_pick(GS_CS_709_SCRGB + 1, past_end, GS_CS_709_SCRGB + 1);
	check_pick_line(-7, 0, NULL, -7, __LINE__);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(route_every_combination),
		cmocka_unit_test(route_other_types_as_inputs),
		cmocka_unit_test(route_other_flags_ignored),
		cmocka_unit_test(route_filter_chains),
		cmocka_unit_test(route_filter_flagged_async),
		cmocka_unit_test(route_no_data_nowhere_to_go),
		cmocka_unit_test(pick_empty_list),
		cmocka_unit_test(pick_match_first),
		cmocka_unit_test(pick_match_last),
		cmocka_unit_test(pick_match_in_the_middle),
		cmocka_unit_test(pick_no_match),
		cmocka_unit_test(pick_out_of_range_passed_through),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
