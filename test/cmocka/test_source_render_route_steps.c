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
#include "obs-source-render-route.h"

/*
 * Characterization of the source and filter render routing of
 * libobs/obs-source.c (source_render_video(), obs_source_main_render(),
 * can_bypass(), obs_source_process_filter_begin_with_color_space(),
 * obs_source_process_filter_tech_end() and obs_source_skip_video_filter()),
 * moved to libobs/obs-source-render-route.c before its Rust port:
 * - obs_source_render_route_filter(): the target and parent check, no target
 *   first (#2177's guard in obs_source_skip_video_filter());
 * - the "no video", async update and inactive checks of
 *   source_render_video(), and which fields each one reads;
 * - obs_source_render_route_draw(), _draw_next() and _skip(): the draw of a
 *   source, of a skipped filter's target and of a filter's target into its
 *   texrender, one value read at a time in the old order;
 * - the default-effect, sRGB-aware and custom draw rules of
 *   obs_source_main_render();
 * - can_bypass() (when gs_get_color_space() is read, and the bypass), the
 *   skip at zero size and the texrender format check;
 * - the technique name, the linear sRGB flag and the bypass or texture draw
 *   of obs_source_process_filter_tech_end().
 * Every boolean input is taken both ways, every source type and colour space
 * (and values outside the enums), every combination of the output flags the
 * rules read, and the NULL parent and target cases. Cases marked
 * "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them. test_filter_color_space, test_filter_skip_removed,
 * test_scroll_filter_wide_source, test_color_correction_16f,
 * test_darken_transparent, test_scene_rotated_scale_filter,
 * test_slideshow_size and test_texture_render cover the callers through the
 * real libobs.
 */

/* source types: the enum and values outside it */
static const int TYPES[] = {OBS_SOURCE_TYPE_INPUT,
			    OBS_SOURCE_TYPE_FILTER,
			    OBS_SOURCE_TYPE_TRANSITION,
			    OBS_SOURCE_TYPE_SCENE,
			    -1,
			    4,
			    99,
			    INT_MIN,
			    INT_MAX};
#define TYPE_COUNT (sizeof(TYPES) / sizeof(TYPES[0]))

/* colour spaces: the enum and values outside it */
static const int SPACES[] = {GS_CS_SRGB, GS_CS_SRGB_16F, GS_CS_709_EXTENDED, GS_CS_709_SCRGB, -1, 4, INT_MIN, INT_MAX};
#define SPACE_COUNT (sizeof(SPACES) / sizeof(SPACES[0]))

/* the direct render choice: the enum and values outside it */
static const int ALLOW[] = {OBS_NO_DIRECT_RENDERING, OBS_ALLOW_DIRECT_RENDERING, 2, -1, INT_MIN, INT_MAX};
#define ALLOW_COUNT (sizeof(ALLOW) / sizeof(ALLOW[0]))

/* The output flags the rules read, and two they do not: every combination of
 * these 6 bits is taken, then UINT32_MAX. */
static const uint32_t FLAG_BITS[] = {OBS_SOURCE_VIDEO, OBS_SOURCE_ASYNC, OBS_SOURCE_CUSTOM_DRAW,
				     OBS_SOURCE_SRGB,  OBS_SOURCE_AUDIO, OBS_SOURCE_CAP_DISABLED};
#define FLAG_BIT_COUNT (sizeof(FLAG_BITS) / sizeof(FLAG_BITS[0]))
#define FLAG_COMBOS ((1u << FLAG_BIT_COUNT) + 1)

static uint32_t flags_of(unsigned combo)
{
	if (combo == (1u << FLAG_BIT_COUNT))
		return UINT32_MAX;
	uint32_t flags = 0;
	for (size_t b = 0; b < FLAG_BIT_COUNT; b++)
		if (combo & (1u << b))
			flags |= FLAG_BITS[b];
	return flags;
}

static void check_int(long long actual, long long expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %lld, expected %lld\n", line, what, actual, expected);
		fail();
	}
}

static void check_bool(bool actual, bool expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %s, expected %s\n", line, what, actual ? "true" : "false",
			expected ? "true" : "false");
		fail();
	}
}

#define CHECK_INT(actual, expected) check_int((long long)(actual), (long long)(expected), #actual, __LINE__)
#define CHECK_BOOL(actual, expected) check_bool((actual), (expected), #actual, __LINE__)

/* --- enum values ------------------------------------------------------- */

/* The Rust port returns these numbers; the C acts on them. */
static void enum_values(void **state)
{
	(void)state;
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_FILTER_READY, 0);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_FILTER_NO_TARGET, 1);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_FILTER_NO_PARENT, 2);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT, 0);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS, 1);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN, 2);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET, 3);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE, 4);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC, 5);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER, 6);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER, 7);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET, 8);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING, 9);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER, 10);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING, 11);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_TECH_END_BYPASS, 0);
	CHECK_INT(OBS_SOURCE_RENDER_ROUTE_TECH_END_TEXTURE, 1);
	CHECK_INT(sizeof(enum obs_source_render_route_filter), sizeof(int));
	CHECK_INT(sizeof(enum obs_source_render_route_draw), sizeof(int));
	CHECK_INT(sizeof(enum obs_source_render_route_tech_end), sizeof(int));
	/* obs-source.c passes these as int */
	CHECK_INT(sizeof(enum obs_source_type), sizeof(int));
	CHECK_INT(sizeof(enum obs_allow_direct_render), sizeof(int));
	CHECK_INT(sizeof(enum gs_color_space), sizeof(int));
	CHECK_INT(sizeof(enum gs_color_format), sizeof(int));
}

/* --- the target and parent check ----------------------------------------- */

/* No target is checked first, then no parent (#2177: a filter removed while
 * it renders has neither, and draws nothing). */
static void filter_target_and_parent(void **state)
{
	(void)state;
	CHECK_INT(obs_source_render_route_filter(true, true), OBS_SOURCE_RENDER_ROUTE_FILTER_READY);
	CHECK_INT(obs_source_render_route_filter(false, true), OBS_SOURCE_RENDER_ROUTE_FILTER_NO_TARGET);
	CHECK_INT(obs_source_render_route_filter(true, false), OBS_SOURCE_RENDER_ROUTE_FILTER_NO_PARENT);
	CHECK_INT(obs_source_render_route_filter(false, false), OBS_SOURCE_RENDER_ROUTE_FILTER_NO_TARGET);
}

/* --- source_render_video(): the checks before the draw -------------------- */

/* The old check: type != FILTER && (output_flags & VIDEO) == 0, the flags read
 * only for a type that is not a filter. A type outside the enum is not a
 * filter. */
static void video_none_every_type_and_flags(void **state)
{
	(void)state;
	for (size_t t = 0; t < TYPE_COUNT; t++) {
		const bool filter = TYPES[t] == OBS_SOURCE_TYPE_FILTER;
		CHECK_BOOL(obs_source_render_route_video_reads_flags(TYPES[t]), !filter);
		for (unsigned c = 0; c < FLAG_COMBOS; c++) {
			const uint32_t flags = flags_of(c);
			CHECK_BOOL(obs_source_render_route_video_none(TYPES[t], flags),
				   !filter && (flags & OBS_SOURCE_VIDEO) == 0);
		}
	}
}

/* A filter draws whatever its flags (a filter in a chain must render or skip
 * itself), and an audio-only source of any other type draws nothing. */
static void video_none_filter_and_audio(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_render_route_video_none(OBS_SOURCE_TYPE_FILTER, 0), false);
	CHECK_BOOL(obs_source_render_route_video_none(OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_AUDIO), false);
	CHECK_BOOL(obs_source_render_route_video_none(OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_AUDIO), true);
	CHECK_BOOL(obs_source_render_route_video_none(OBS_SOURCE_TYPE_SCENE, 0), true);
	CHECK_BOOL(obs_source_render_route_video_none(OBS_SOURCE_TYPE_TRANSITION, OBS_SOURCE_VIDEO), false);
	CHECK_BOOL(obs_source_render_route_video_none(OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_ASYNC_VIDEO), false);
	/* characterized, not endorsed: ASYNC without VIDEO is no video */
	CHECK_BOOL(obs_source_render_route_video_none(OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_ASYNC), true);
}

/* The old check: type == INPUT && (output_flags & ASYNC) &&
 * !rendering_filter; the flags read only for an input and rendering_filter
 * only for an async one. */
static void async_update_every_type_flags_and_filter(void **state)
{
	(void)state;
	for (size_t t = 0; t < TYPE_COUNT; t++) {
		const bool input = TYPES[t] == OBS_SOURCE_TYPE_INPUT;
		CHECK_BOOL(obs_source_render_route_async_reads_flags(TYPES[t]), input);
		for (unsigned c = 0; c < FLAG_COMBOS; c++) {
			const uint32_t flags = flags_of(c);
			const bool async = input && (flags & OBS_SOURCE_ASYNC) != 0;
			CHECK_BOOL(obs_source_render_route_async_reads_rendering_filter(TYPES[t], flags), async);
			for (int rf = 0; rf < 2; rf++)
				CHECK_BOOL(obs_source_render_route_async_update(TYPES[t], flags, rf != 0),
					   async && !rf);
		}
	}
}

/* An async input drawn by its own filter chain updates its textures once, on
 * the outer call (obs_source_render_filters() sets rendering_filter). A
 * scene, a transition or a filter with the ASYNC flag never updates. */
static void async_update_cases(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_render_route_async_update(OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_ASYNC_VIDEO, false), true);
	CHECK_BOOL(obs_source_render_route_async_update(OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_ASYNC_VIDEO, true), false);
	CHECK_BOOL(obs_source_render_route_async_update(OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_VIDEO, false), false);
	CHECK_BOOL(obs_source_render_route_async_update(OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_ASYNC_VIDEO, false), false);
	CHECK_BOOL(obs_source_render_route_async_update(OBS_SOURCE_TYPE_SCENE, OBS_SOURCE_ASYNC_VIDEO, false), false);
	CHECK_BOOL(obs_source_render_route_async_update(OBS_SOURCE_TYPE_TRANSITION, OBS_SOURCE_ASYNC_VIDEO, false),
		   false);
	/* characterized, not endorsed: ASYNC without VIDEO still updates (the
	 * "no video" check returned before for such a source) */
	CHECK_BOOL(obs_source_render_route_async_update(OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_ASYNC, false), true);
}

/* The old check: !context.data || !enabled, enabled read only with data. */
static void inactive_every_input(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_render_route_reads_enabled(false), false);
	CHECK_BOOL(obs_source_render_route_reads_enabled(true), true);
	CHECK_BOOL(obs_source_render_route_inactive(false, false), true);
	CHECK_BOOL(obs_source_render_route_inactive(false, true), true);
	CHECK_BOOL(obs_source_render_route_inactive(true, false), true);
	CHECK_BOOL(obs_source_render_route_inactive(true, true), false);
}

/* --- the draw, one read at a time ---------------------------------------- */

/* What the old chain read and drew, as a READ_ step sequence. */
struct walk {
	enum obs_source_render_route_draw route;
	/* the READ_ steps asked for, in order */
	int reads[8];
	size_t read_count;
};

static bool step_value(enum obs_source_render_route_draw step, const bool values[6])
{
	switch (step) {
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER:
		return values[0];
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER:
		return values[1];
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET:
		return values[2];
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING:
		return values[3];
	default:
		fprintf(stderr, "step %d is not a read\n", (int)step);
		fail();
	}
	return false;
}

static bool is_read(enum obs_source_render_route_draw step)
{
	return step >= OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER &&
	       step <= OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING;
}

/* Walks from first, reading values[] (rendering_filter, video_render,
 * filter_target, deinterlacing) as the C does. */
static struct walk walk_route(enum obs_source_render_route_draw first, const bool values[6])
{
	struct walk w = {first, {0}, 0};
	while (is_read(w.route)) {
		if (w.read_count == sizeof(w.reads) / sizeof(w.reads[0])) {
			fprintf(stderr, "route does not end\n");
			fail();
		}
		w.reads[w.read_count++] = (int)w.route;
		w.route = obs_source_render_route_draw_next(w.route, step_value(w.route, values));
	}
	return w;
}

static void check_reads(const struct walk *w, const int *expected, size_t count, int line)
{
	check_int((long long)w->read_count, (long long)count, "read count", line);
	for (size_t i = 0; i < count; i++)
		check_int(w->reads[i], expected[i], "read", line);
}

/* source_render_video()'s draw for every combination of has_filters,
 * rendering_filter, video_render, filter_target and deinterlacing, against
 * the old chain: filters && !rendering_filter -> filters, else video_render
 * -> main, else filter_target -> target, else deinterlacing -> deinterlace,
 * else async; each value read only when the chain gets to it. */
static void draw_every_source(void **state)
{
	(void)state;
	for (unsigned m = 0; m < 32; m++) {
		const bool has_filters = (m & 1) != 0;
		const bool values[6] = {(m & 2) != 0, (m & 4) != 0, (m & 8) != 0, (m & 16) != 0};
		const bool rendering_filter = values[0], video_render = values[1], filter_target = values[2],
			   deinterlacing = values[3];

		struct walk w = walk_route(obs_source_render_route_draw(has_filters), values);

		int expected_reads[4];
		size_t n = 0;
		enum obs_source_render_route_draw expected;
		if (has_filters)
			expected_reads[n++] = OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER;
		if (has_filters && !rendering_filter) {
			expected = OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS;
		} else {
			expected_reads[n++] = OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER;
			if (video_render) {
				expected = OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN;
			} else {
				expected_reads[n++] = OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET;
				if (filter_target) {
					expected = OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET;
				} else {
					expected_reads[n++] = OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING;
					expected = deinterlacing ? OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE
								 : OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC;
				}
			}
		}
		CHECK_INT(w.route, expected);
		check_reads(&w, expected_reads, n, __LINE__);
	}
}

/* Each step on its own: both values for every READ_ step; a draw and values
 * outside the enum come back unchanged. */
static void draw_next_every_step(void **state)
{
	(void)state;
	CHECK_INT(obs_source_render_route_draw(true), OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER);
	CHECK_INT(obs_source_render_route_draw(false), OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER);

	static const struct {
		enum obs_source_render_route_draw step;
		enum obs_source_render_route_draw if_false, if_true;
	} steps[] = {
		{OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER, OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS,
		 OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER},
		{OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER, OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET,
		 OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN},
		{OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET, OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING,
		 OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET},
		{OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING, OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC,
		 OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE},
		{OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER,
		 OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING, OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN},
		{OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING, OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC,
		 OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE},
	};
	for (size_t i = 0; i < sizeof(steps) / sizeof(steps[0]); i++) {
		CHECK_INT(obs_source_render_route_draw_next(steps[i].step, false), steps[i].if_false);
		CHECK_INT(obs_source_render_route_draw_next(steps[i].step, true), steps[i].if_true);
	}

	static const int unchanged[] = {OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT,
					OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS,
					OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN,
					OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET,
					OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE,
					OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC,
					-1,
					12,
					99,
					INT_MIN,
					INT_MAX};
	for (size_t i = 0; i < sizeof(unchanged) / sizeof(unchanged[0]); i++)
		for (int v = 0; v < 2; v++)
			/* as int: the enum is unsigned on some compilers */
			CHECK_INT((int)obs_source_render_route_draw_next(
					  (enum obs_source_render_route_draw)unchanged[i], v != 0),
				  unchanged[i]);
}

/* obs_source_skip_video_filter()'s draw for every parent flag combination,
 * the target being the parent or not, video_render and deinterlacing,
 * against the old chain: the target is the parent -> (neither custom draw nor
 * async -> default render, else video_render -> main, else deinterlacing ->
 * deinterlace, else async), else the target's video render. */
static void skip_every_target_and_flags(void **state)
{
	(void)state;
	for (int same = 0; same < 2; same++)
		for (unsigned c = 0; c < FLAG_COMBOS; c++)
			for (unsigned m = 0; m < 4; m++) {
				const uint32_t flags = flags_of(c);
				const bool custom_draw = (flags & OBS_SOURCE_CUSTOM_DRAW) != 0;
				const bool async = (flags & OBS_SOURCE_ASYNC) != 0;
				const bool values[6] = {false, (m & 1) != 0, false, (m & 2) != 0};

				struct walk w = walk_route(obs_source_render_route_skip(same != 0, flags), values);

				int expected_reads[4];
				size_t n = 0;
				enum obs_source_render_route_draw expected;
				if (!same) {
					expected = OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET;
				} else if (!custom_draw && !async) {
					expected = OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT;
				} else {
					expected_reads[n++] = OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER;
					if (values[1]) {
						expected = OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN;
					} else {
						expected_reads[n++] =
							OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING;
						expected = values[3] ? OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE
								     : OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC;
					}
				}
				CHECK_INT(w.route, expected);
				check_reads(&w, expected_reads, n, __LINE__);
			}
}

/* A skipped filter whose target is another filter hands the draw to it, even
 * for a custom draw or async parent; a plain parent draws with the default
 * effect. */
static void skip_cases(void **state)
{
	(void)state;
	CHECK_INT(obs_source_render_route_skip(false, OBS_SOURCE_VIDEO), OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET);
	CHECK_INT(obs_source_render_route_skip(false, OBS_SOURCE_CUSTOM_DRAW), OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET);
	CHECK_INT(obs_source_render_route_skip(true, OBS_SOURCE_VIDEO), OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT);
	CHECK_INT(obs_source_render_route_skip(true, OBS_SOURCE_ASYNC_VIDEO),
		  OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER);
	CHECK_INT(obs_source_render_route_skip(true, OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW),
		  OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER);
	/* the SRGB flag plays no part */
	CHECK_INT(obs_source_render_route_skip(true, OBS_SOURCE_VIDEO | OBS_SOURCE_SRGB),
		  OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT);
}

/* --- obs_source_main_render() --------------------------------------------- */

/* The old rules: custom_draw and srgb_aware from the flags, default_effect =
 * !filter_parent && filters.num == 0 && !custom_draw, filters.num read only
 * for a source that is not a filter. */
static void main_render_every_input(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_render_route_main_reads_filters(false), true);
	CHECK_BOOL(obs_source_render_route_main_reads_filters(true), false);
	for (unsigned c = 0; c < FLAG_COMBOS; c++) {
		const uint32_t flags = flags_of(c);
		const bool custom_draw = (flags & OBS_SOURCE_CUSTOM_DRAW) != 0;
		CHECK_BOOL(obs_source_render_route_main_custom_draw(flags), custom_draw);
		CHECK_BOOL(obs_source_render_route_main_srgb_aware(flags), (flags & OBS_SOURCE_SRGB) != 0);
		for (int parent = 0; parent < 2; parent++)
			for (int filters = 0; filters < 2; filters++)
				CHECK_BOOL(obs_source_render_route_main_default_effect(flags, parent != 0,
										       filters != 0),
					   !parent && !filters && !custom_draw);
	}
}

/* --- obs_source_process_filter_begin_with_color_space() --------------------- */

/* can_bypass(), for every wiring, direct render choice, pair of filter and
 * parent flags and pair of spaces: (target == parent) && allow_direct ==
 * OBS_ALLOW_DIRECT_RENDERING && !custom draw && !async parent && the SRGB
 * flags agree, and only then gs_get_color_space() is read and compared. */
static void bypass_every_input(void **state)
{
	(void)state;
	for (int same = 0; same < 2; same++)
		for (size_t a = 0; a < ALLOW_COUNT; a++)
			for (unsigned fc = 0; fc < FLAG_COMBOS; fc++)
				for (unsigned pc = 0; pc < FLAG_COMBOS; pc++) {
					const uint32_t ff = flags_of(fc), pf = flags_of(pc);
					const bool reads = same && ALLOW[a] == OBS_ALLOW_DIRECT_RENDERING &&
							   (pf & OBS_SOURCE_CUSTOM_DRAW) == 0 &&
							   (pf & OBS_SOURCE_ASYNC) == 0 &&
							   (ff & OBS_SOURCE_SRGB) == (pf & OBS_SOURCE_SRGB);
					CHECK_BOOL(obs_source_render_route_bypass_reads_space(same != 0, ALLOW[a], ff,
											      pf),
						   reads);
					for (size_t s = 0; s < SPACE_COUNT; s++)
						for (size_t cs = 0; cs < SPACE_COUNT; cs++)
							CHECK_BOOL(obs_source_render_route_bypass(same != 0, ALLOW[a],
												  ff, pf, SPACES[s],
												  SPACES[cs]),
								   reads && SPACES[s] == SPACES[cs]);
				}
}

/* The colour space rules of #1493's class: a filter and a parent that differ
 * on OBS_SOURCE_SRGB, or a space other than the current one, go through the
 * texrender; a direct render keeps the space. */
static void bypass_cases(void **state)
{
	(void)state;
	const int direct = OBS_ALLOW_DIRECT_RENDERING;
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, 0, OBS_SOURCE_VIDEO, GS_CS_SRGB, GS_CS_SRGB), true);
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, OBS_SOURCE_SRGB, OBS_SOURCE_VIDEO | OBS_SOURCE_SRGB,
						  GS_CS_709_EXTENDED, GS_CS_709_EXTENDED),
		   true);
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, OBS_SOURCE_SRGB, OBS_SOURCE_VIDEO, GS_CS_SRGB,
						  GS_CS_SRGB),
		   false);
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, 0, OBS_SOURCE_VIDEO | OBS_SOURCE_SRGB, GS_CS_SRGB,
						  GS_CS_SRGB),
		   false);
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, 0, OBS_SOURCE_VIDEO, GS_CS_SRGB, GS_CS_709_SCRGB),
		   false);
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, 0, OBS_SOURCE_VIDEO, GS_CS_SRGB_16F, GS_CS_SRGB),
		   false);
	CHECK_BOOL(obs_source_render_route_bypass(false, direct, 0, OBS_SOURCE_VIDEO, GS_CS_SRGB, GS_CS_SRGB), false);
	CHECK_BOOL(obs_source_render_route_bypass(true, OBS_NO_DIRECT_RENDERING, 0, OBS_SOURCE_VIDEO, GS_CS_SRGB,
						  GS_CS_SRGB),
		   false);
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, 0, OBS_SOURCE_ASYNC_VIDEO, GS_CS_SRGB, GS_CS_SRGB),
		   false);
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, 0, OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
						  GS_CS_SRGB, GS_CS_SRGB),
		   false);
	/* characterized, not endorsed: a direct render choice outside the enum
	 * is not direct; spaces outside the enum bypass when they are equal */
	CHECK_BOOL(obs_source_render_route_bypass(true, 2, 0, OBS_SOURCE_VIDEO, GS_CS_SRGB, GS_CS_SRGB), false);
	CHECK_BOOL(obs_source_render_route_bypass(true, direct, 0, OBS_SOURCE_VIDEO, 99, 99), true);
	/* the current space is not looked at when the rest does not hold */
	CHECK_BOOL(obs_source_render_route_bypass(false, direct, 0, OBS_SOURCE_VIDEO, GS_CS_SRGB, INT_MIN), false);
}

/* The skip at zero size: either size 0. Characterized, not endorsed: the
 * sizes are get_base_width()'s and get_base_height()'s uint32_t in an int, so
 * a size of 2^31 or more is negative there and does not skip. */
static void size_skip_every_size(void **state)
{
	(void)state;
	static const int sizes[] = {0, 1, 2, 1080, 1920, -1, INT_MIN, INT_MAX};
	for (size_t x = 0; x < sizeof(sizes) / sizeof(sizes[0]); x++)
		for (size_t y = 0; y < sizeof(sizes) / sizeof(sizes[0]); y++)
			CHECK_BOOL(obs_source_render_route_size_skip(sizes[x], sizes[y]),
				   sizes[x] == 0 || sizes[y] == 0);
}

/* The texrender is kept for the same format and replaced for any other, for
 * every pair of formats in the enum and around it. */
static void texrender_stale_every_format(void **state)
{
	(void)state;
	for (int a = -2; a <= GS_RG16 + 2; a++)
		for (int b = -2; b <= GS_RG16 + 2; b++)
			CHECK_BOOL(obs_source_render_route_texrender_stale(a, b), a != b);
	CHECK_BOOL(obs_source_render_route_texrender_stale(INT_MIN, INT_MIN), false);
	CHECK_BOOL(obs_source_render_route_texrender_stale(INT_MIN, INT_MAX), true);
}

/* The draw into the texrender: the default render for the parent itself when
 * it is neither custom draw nor async, else the target's video render. */
static void filter_render_every_input(void **state)
{
	(void)state;
	for (int same = 0; same < 2; same++)
		for (unsigned c = 0; c < FLAG_COMBOS; c++) {
			const uint32_t flags = flags_of(c);
			const bool plain = (flags & OBS_SOURCE_CUSTOM_DRAW) == 0 && (flags & OBS_SOURCE_ASYNC) == 0;
			CHECK_INT(obs_source_render_route_filter_render(same != 0, flags),
				  same && plain ? OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT
						: OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET);
		}
}

/* --- obs_source_process_filter_tech_end() --------------------------------- */

/* The filter's OBS_SOURCE_SRGB sets the linear sRGB flag; bypass or texture
 * as begin decided; the technique "Draw" when none is given, else the given
 * pointer itself. */
static void tech_end_every_input(void **state)
{
	(void)state;
	for (unsigned c = 0; c < FLAG_COMBOS; c++) {
		const uint32_t flags = flags_of(c);
		CHECK_BOOL(obs_source_render_route_tech_end_linear_srgb(flags), (flags & OBS_SOURCE_SRGB) != 0);
	}
	CHECK_INT(obs_source_render_route_tech_end(true), OBS_SOURCE_RENDER_ROUTE_TECH_END_BYPASS);
	CHECK_INT(obs_source_render_route_tech_end(false), OBS_SOURCE_RENDER_ROUTE_TECH_END_TEXTURE);

	const char *draw = obs_source_render_route_tech_name(NULL);
	assert_non_null(draw);
	assert_string_equal(draw, "Draw");
	static const char custom[] = "DrawCustom";
	static const char empty[] = "";
	assert_ptr_equal(obs_source_render_route_tech_name(custom), custom);
	/* characterized, not endorsed: an empty name is passed on as it is */
	assert_ptr_equal(obs_source_render_route_tech_name(empty), empty);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enum_values),
		cmocka_unit_test(filter_target_and_parent),
		cmocka_unit_test(video_none_every_type_and_flags),
		cmocka_unit_test(video_none_filter_and_audio),
		cmocka_unit_test(async_update_every_type_flags_and_filter),
		cmocka_unit_test(async_update_cases),
		cmocka_unit_test(inactive_every_input),
		cmocka_unit_test(draw_every_source),
		cmocka_unit_test(draw_next_every_step),
		cmocka_unit_test(skip_every_target_and_flags),
		cmocka_unit_test(skip_cases),
		cmocka_unit_test(main_render_every_input),
		cmocka_unit_test(bypass_every_input),
		cmocka_unit_test(bypass_cases),
		cmocka_unit_test(size_skip_every_size),
		cmocka_unit_test(texrender_stale_every_format),
		cmocka_unit_test(filter_render_every_input),
		cmocka_unit_test(tech_end_every_input),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
