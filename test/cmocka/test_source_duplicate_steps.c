#include <limits.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-duplicate.h"

/*
 * Characterization of the duplicate decisions of obs_source_duplicate() and
 * get_new_filter_name() in libobs/obs-source.c, and of obs_scene_duplicate(),
 * dup_child() and dup_group() in libobs/obs-scene.c, moved to
 * libobs/obs-source-duplicate.c before their Rust port:
 * - the route of obs_source_duplicate(), one step after each lookup or read
 *   (type, scene, group, flags and created steps), the duplicate type of a
 *   scene copy and the filter copy;
 * - the number get_new_filter_name() tries next;
 * - the flags of obs_scene_duplicate(), its group override, its per-item
 *   route, the reuse step of dup_child(), the name of a copy and the group
 *   resize.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. Sources are addresses the functions only
 * compare, never follow. Models of the callers below use them the way
 * obs-source.c and obs-scene.c do.
 *
 * Not pinned here, because the C has no check there: a create that returns
 * no source (obs_source_duplicate() writes through it), and a repeated
 * source whose earlier item was not added to the new scene (dup_child()
 * takes the new scene's item at the old index, which is then another
 * item's).
 */

static char storage[8];
#define SRC(i) ((const void *)&storage[i])

#define ARRAY_LEN(a) (sizeof(a) / sizeof((a)[0]))

#define TYPE_INPUT 0
#define TYPE_FILTER OBS_SOURCE_DUPLICATE_TYPE_FILTER
#define TYPE_TRANSITION 2
#define TYPE_SCENE OBS_SOURCE_DUPLICATE_TYPE_SCENE

/* ------------------------------------------------------------------------ */
/* obs_source_duplicate() */

struct source_model {
	int type;
	/* obs_scene_from_source() finds a scene */
	bool scene;
	/* obs_group_from_source() finds a group */
	bool group;
	uint32_t output_flags;
};

enum outcome {
	OUT_NULL,
	OUT_REF,
	OUT_SCENE_COPY,
	OUT_SCENE_PRIVATE_COPY,
	OUT_CREATE_WITH_FILTERS,
	OUT_CREATE_NO_FILTERS,
};

struct asks {
	int scene_lookups;
	int group_lookups;
	int flag_reads;
	int creates;
};

/* A model of obs_source_duplicate(): each lookup and read only where the
 * step asks for it. */
static enum outcome duplicate_route(struct source_model s, bool create_private, struct asks *asks)
{
	memset(asks, 0, sizeof(*asks));

	if (obs_source_duplicate_type_step(s.type) == OBS_SOURCE_DUPLICATE_ASK_SCENE) {
		asks->scene_lookups++;
		bool found = s.scene;
		int step = obs_source_duplicate_scene_step(found, create_private);
		if (step == OBS_SOURCE_DUPLICATE_REF)
			return OUT_REF;
		if (step == OBS_SOURCE_DUPLICATE_ASK_GROUP) {
			asks->group_lookups++;
			found = s.group;
			step = obs_source_duplicate_group_step(found);
		}
		if (step != OBS_SOURCE_DUPLICATE_SCENE_COPY)
			return OUT_NULL;

		int type = obs_source_duplicate_scene_type(create_private);
		return type == OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY ? OUT_SCENE_PRIVATE_COPY : OUT_SCENE_COPY;
	}

	asks->flag_reads++;
	if (obs_source_duplicate_flags_step(s.output_flags) == OBS_SOURCE_DUPLICATE_REF)
		return OUT_REF;

	asks->creates++;
	if (obs_source_duplicate_created_step(true) != OBS_SOURCE_DUPLICATE_COPY_STATE)
		return OUT_NULL;

	return obs_source_duplicate_copies_filters(s.type) ? OUT_CREATE_WITH_FILTERS : OUT_CREATE_NO_FILTERS;
}

static void type_step(void **state)
{
	(void)state;

	assert_int_equal(obs_source_duplicate_type_step(TYPE_SCENE), OBS_SOURCE_DUPLICATE_ASK_SCENE);
	assert_int_equal(obs_source_duplicate_type_step(TYPE_INPUT), OBS_SOURCE_DUPLICATE_ASK_FLAGS);
	assert_int_equal(obs_source_duplicate_type_step(TYPE_FILTER), OBS_SOURCE_DUPLICATE_ASK_FLAGS);
	assert_int_equal(obs_source_duplicate_type_step(TYPE_TRANSITION), OBS_SOURCE_DUPLICATE_ASK_FLAGS);
	/* any other value is not a scene */
	assert_int_equal(obs_source_duplicate_type_step(-1), OBS_SOURCE_DUPLICATE_ASK_FLAGS);
	assert_int_equal(obs_source_duplicate_type_step(4), OBS_SOURCE_DUPLICATE_ASK_FLAGS);
	assert_int_equal(obs_source_duplicate_type_step(INT_MAX), OBS_SOURCE_DUPLICATE_ASK_FLAGS);
}

static void scene_steps(void **state)
{
	(void)state;

	/* a public duplicate of a scene shares it; a private one copies it */
	assert_int_equal(obs_source_duplicate_scene_step(true, false), OBS_SOURCE_DUPLICATE_REF);
	assert_int_equal(obs_source_duplicate_scene_step(true, true), OBS_SOURCE_DUPLICATE_SCENE_COPY);
	/* no scene: look for a group, whichever kind is asked */
	assert_int_equal(obs_source_duplicate_scene_step(false, false), OBS_SOURCE_DUPLICATE_ASK_GROUP);
	assert_int_equal(obs_source_duplicate_scene_step(false, true), OBS_SOURCE_DUPLICATE_ASK_GROUP);

	assert_int_equal(obs_source_duplicate_group_step(true), OBS_SOURCE_DUPLICATE_SCENE_COPY);
	assert_int_equal(obs_source_duplicate_group_step(false), OBS_SOURCE_DUPLICATE_NONE);

	assert_int_equal(obs_source_duplicate_scene_type(false), OBS_SOURCE_DUPLICATE_DUP_COPY);
	assert_int_equal(obs_source_duplicate_scene_type(true), OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY);
}

static void flags_and_filters(void **state)
{
	(void)state;

	assert_int_equal(obs_source_duplicate_flags_step(0), OBS_SOURCE_DUPLICATE_CREATE);
	assert_int_equal(obs_source_duplicate_flags_step(OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE),
			 OBS_SOURCE_DUPLICATE_REF);
	/* only that bit counts */
	assert_int_equal(obs_source_duplicate_flags_step(~(uint32_t)OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE),
			 OBS_SOURCE_DUPLICATE_CREATE);
	assert_int_equal(obs_source_duplicate_flags_step(UINT32_MAX), OBS_SOURCE_DUPLICATE_REF);
	assert_int_equal(obs_source_duplicate_flags_step(0x3 | OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE),
			 OBS_SOURCE_DUPLICATE_REF);

	/* a source was created: copy its state */
	assert_int_equal(obs_source_duplicate_created_step(true), OBS_SOURCE_DUPLICATE_COPY_STATE);

	assert_true(obs_source_duplicate_copies_filters(TYPE_INPUT));
	assert_true(obs_source_duplicate_copies_filters(TYPE_TRANSITION));
	assert_true(obs_source_duplicate_copies_filters(TYPE_SCENE));
	assert_false(obs_source_duplicate_copies_filters(TYPE_FILTER));
	assert_true(obs_source_duplicate_copies_filters(-1));
}

struct route_case {
	const char *what;
	struct source_model source;
	bool create_private;
	enum outcome outcome;
	struct asks asks;
};

static void source_routes(void **state)
{
	(void)state;

	const struct route_case cases[] = {
		{"public scene", {TYPE_SCENE, true, false, 0}, false, OUT_REF, {1, 0, 0, 0}},
		{"private scene", {TYPE_SCENE, true, false, 0}, true, OUT_SCENE_PRIVATE_COPY, {1, 0, 0, 0}},
		{"public group", {TYPE_SCENE, false, true, 0}, false, OUT_SCENE_COPY, {1, 1, 0, 0}},
		{"private group", {TYPE_SCENE, false, true, 0}, true, OUT_SCENE_PRIVATE_COPY, {1, 1, 0, 0}},
		/* typed as a scene, but neither a scene nor a group */
		{"unknown scene", {TYPE_SCENE, false, false, 0}, false, OUT_NULL, {1, 1, 0, 0}},
		{"unknown scene private", {TYPE_SCENE, false, false, 0}, true, OUT_NULL, {1, 1, 0, 0}},
		/* a scene's flags are never read */
		{"scene no dup",
		 {TYPE_SCENE, true, false, OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE},
		 true,
		 OUT_SCENE_PRIVATE_COPY,
		 {1, 0, 0, 0}},
		{"input", {TYPE_INPUT, false, false, 0}, false, OUT_CREATE_WITH_FILTERS, {0, 0, 1, 1}},
		{"input private", {TYPE_INPUT, false, false, 0}, true, OUT_CREATE_WITH_FILTERS, {0, 0, 1, 1}},
		{"input no dup",
		 {TYPE_INPUT, false, false, OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE},
		 false,
		 OUT_REF,
		 {0, 0, 1, 0}},
		{"input no dup private",
		 {TYPE_INPUT, false, false, OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE},
		 true,
		 OUT_REF,
		 {0, 0, 1, 0}},
		{"filter", {TYPE_FILTER, false, false, 0}, true, OUT_CREATE_NO_FILTERS, {0, 0, 1, 1}},
		{"filter no dup",
		 {TYPE_FILTER, false, false, OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE},
		 true,
		 OUT_REF,
		 {0, 0, 1, 0}},
		{"transition", {TYPE_TRANSITION, false, false, 0}, false, OUT_CREATE_WITH_FILTERS, {0, 0, 1, 1}},
	};

	for (size_t i = 0; i < ARRAY_LEN(cases); i++) {
		struct asks asks;
		enum outcome got = duplicate_route(cases[i].source, cases[i].create_private, &asks);
		if (got != cases[i].outcome || memcmp(&asks, &cases[i].asks, sizeof(asks)) != 0) {
			fprintf(stderr, "%s: outcome %d (want %d), asks %d %d %d %d\n", cases[i].what, (int)got,
				(int)cases[i].outcome, asks.scene_lookups, asks.group_lookups, asks.flag_reads,
				asks.creates);
			fail();
		}
	}
}

/* ------------------------------------------------------------------------ */
/* get_new_filter_name() */

/* A model of get_new_filter_name(): the name, then "<name> N" with the
 * number the core gives, until no filter has it. */
static void new_filter_name(char *out, size_t size, const char *name, const char *const *taken, size_t num_taken)
{
	int inc = 0;

	snprintf(out, size, "%s", name);
	for (;;) {
		bool found = false;
		for (size_t i = 0; i < num_taken; i++)
			found = found || strcmp(taken[i], out) == 0;
		if (!found)
			break;
		snprintf(out, size, "%s %d", name, obs_source_duplicate_filter_number(++inc));
	}
}

static void filter_numbers(void **state)
{
	(void)state;

	assert_int_equal(obs_source_duplicate_filter_number(1), 2);
	assert_int_equal(obs_source_duplicate_filter_number(2), 3);
	assert_int_equal(obs_source_duplicate_filter_number(3), 4);
	assert_int_equal(obs_source_duplicate_filter_number(99), 100);
	assert_int_equal(obs_source_duplicate_filter_number(INT_MAX - 1), INT_MAX);
}

static void filter_names(void **state)
{
	(void)state;
	char name[64];

	new_filter_name(name, sizeof(name), "Color", NULL, 0);
	assert_string_equal(name, "Color");

	const char *const one[] = {"Color"};
	new_filter_name(name, sizeof(name), "Color", one, ARRAY_LEN(one));
	assert_string_equal(name, "Color 2");

	const char *const three[] = {"Color 3", "Color", "Color 2"};
	new_filter_name(name, sizeof(name), "Color", three, ARRAY_LEN(three));
	assert_string_equal(name, "Color 4");

	/* a gap is taken first */
	const char *const gap[] = {"Color", "Color 3"};
	new_filter_name(name, sizeof(name), "Color", gap, ARRAY_LEN(gap));
	assert_string_equal(name, "Color 2");

	/* the numbers follow the original name, not the last try: "Color 2"
	 * taken as a name gives "Color 2 2" */
	const char *const numbered[] = {"Color 2"};
	new_filter_name(name, sizeof(name), "Color 2", numbered, ARRAY_LEN(numbered));
	assert_string_equal(name, "Color 2 2");
}

/* ------------------------------------------------------------------------ */
/* obs_scene_duplicate() */

struct flags_case {
	int type;
	bool unique;
	bool private_;
	/* make_unique after the group override */
	bool scene_unique;
	bool group_unique;
};

static void scene_flags(void **state)
{
	(void)state;

	const struct flags_case cases[] = {
		{OBS_SOURCE_DUPLICATE_DUP_REFS, false, false, false, false},
		{OBS_SOURCE_DUPLICATE_DUP_COPY, true, false, true, false},
		{OBS_SOURCE_DUPLICATE_DUP_PRIVATE_REFS, false, true, false, false},
		{OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY, true, true, true, true},
		/* outside the enum: neither, and a group never copies */
		{-1, false, false, false, false},
		{4, false, false, false, false},
	};

	for (size_t i = 0; i < ARRAY_LEN(cases); i++) {
		const struct flags_case *c = &cases[i];
		bool unique = obs_scene_duplicate_unique(c->type);
		assert_int_equal(unique, c->unique);
		assert_int_equal(obs_scene_duplicate_private(c->type), c->private_);
		assert_int_equal(obs_scene_duplicate_keeps_unique(unique, false, c->type), c->scene_unique);
		assert_int_equal(obs_scene_duplicate_keeps_unique(unique, true, c->type), c->group_unique);
	}

	/* the override only clears the flag */
	assert_false(obs_scene_duplicate_keeps_unique(false, false, OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY));
	assert_false(obs_scene_duplicate_keeps_unique(false, true, OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY));
	assert_true(obs_scene_duplicate_keeps_unique(true, false, OBS_SOURCE_DUPLICATE_DUP_REFS));
	assert_false(obs_scene_duplicate_keeps_unique(true, true, OBS_SOURCE_DUPLICATE_DUP_REFS));
}

static void item_routes(void **state)
{
	(void)state;

	assert_int_equal(obs_scene_duplicate_item_route(true, true), OBS_SCENE_DUPLICATE_ITEM_GROUP);
	assert_int_equal(obs_scene_duplicate_item_route(true, false), OBS_SCENE_DUPLICATE_ITEM_GROUP);
	assert_int_equal(obs_scene_duplicate_item_route(false, true), OBS_SCENE_DUPLICATE_ITEM_CHILD);
	assert_int_equal(obs_scene_duplicate_item_route(false, false), OBS_SCENE_DUPLICATE_ITEM_REF);
}

static void names_and_resize(void **state)
{
	(void)state;

	assert_true(obs_scene_duplicate_keeps_name(true));
	assert_false(obs_scene_duplicate_keeps_name(false));
	assert_true(obs_scene_duplicate_resizes(true));
	assert_false(obs_scene_duplicate_resizes(false));
}

static void reuse_step(void **state)
{
	(void)state;

	/* the same source, its copy in the new scene: reuse it */
	int same = obs_scene_duplicate_reuse(SRC(1), SRC(1), true);
	assert_true(same == OBS_SCENE_DUPLICATE_REUSE_AT_INDEX || same == OBS_SCENE_DUPLICATE_REUSE_COPY);
	/* another source: go on */
	assert_int_equal(obs_scene_duplicate_reuse(SRC(1), SRC(2), true), OBS_SCENE_DUPLICATE_REUSE_NONE);
	assert_int_equal(obs_scene_duplicate_reuse(SRC(2), SRC(1), false), OBS_SCENE_DUPLICATE_REUSE_NONE);
	assert_int_equal(obs_scene_duplicate_reuse(NULL, SRC(1), true), OBS_SCENE_DUPLICATE_REUSE_NONE);
}

enum copy_kind {
	COPY_GROUP,
	COPY_NEW,
	COPY_REUSE,
	COPY_REF,
};

struct copy_plan {
	enum copy_kind kind;
	/* COPY_REUSE: the old item whose copy is shared */
	size_t from;
};

/* A model of obs_scene_duplicate()'s item loop, with every item added. */
static void scene_copies(int type, bool scene_is_group, const void *const *sources, const bool *is_group, size_t num,
			 struct copy_plan *out)
{
	bool unique = obs_scene_duplicate_unique(type);
	unique = obs_scene_duplicate_keeps_unique(unique, scene_is_group, type);

	for (size_t idx = 0; idx < num; idx++) {
		int route = obs_scene_duplicate_item_route(is_group[idx], unique);
		if (route == OBS_SCENE_DUPLICATE_ITEM_GROUP) {
			out[idx].kind = COPY_GROUP;
		} else if (route == OBS_SCENE_DUPLICATE_ITEM_CHILD) {
			out[idx].kind = COPY_NEW;
			for (size_t i = 0; i < idx; i++) {
				if (obs_scene_duplicate_reuse(sources[i], sources[idx], true) !=
				    OBS_SCENE_DUPLICATE_REUSE_NONE) {
					out[idx].kind = COPY_REUSE;
					out[idx].from = i;
					break;
				}
			}
		} else {
			out[idx].kind = COPY_REF;
		}
	}
}

static void scene_one_source_three_times(void **state)
{
	(void)state;
	const void *const sources[] = {SRC(1), SRC(1), SRC(1)};
	const bool groups[] = {false, false, false};
	struct copy_plan plan[3];

	/* one copy, two references to it */
	scene_copies(OBS_SOURCE_DUPLICATE_DUP_COPY, false, sources, groups, 3, plan);
	assert_int_equal(plan[0].kind, COPY_NEW);
	assert_int_equal(plan[1].kind, COPY_REUSE);
	assert_int_equal(plan[1].from, 0);
	assert_int_equal(plan[2].kind, COPY_REUSE);
	assert_int_equal(plan[2].from, 0);

	/* references only: each item refers to the old source */
	scene_copies(OBS_SOURCE_DUPLICATE_DUP_REFS, false, sources, groups, 3, plan);
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(plan[i].kind, COPY_REF);
}

static void scene_mixed_sources(void **state)
{
	(void)state;
	const void *const sources[] = {SRC(1), SRC(2), SRC(1), SRC(3), SRC(2), SRC(4), SRC(4)};
	const bool groups[] = {false, false, false, false, false, true, true};
	struct copy_plan plan[7];

	scene_copies(OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY, false, sources, groups, 7, plan);
	assert_int_equal(plan[0].kind, COPY_NEW);
	assert_int_equal(plan[1].kind, COPY_NEW);
	assert_int_equal(plan[2].kind, COPY_REUSE);
	assert_int_equal(plan[2].from, 0);
	assert_int_equal(plan[3].kind, COPY_NEW);
	assert_int_equal(plan[4].kind, COPY_REUSE);
	assert_int_equal(plan[4].from, 1);
	/* a group used twice is copied twice */
	assert_int_equal(plan[5].kind, COPY_GROUP);
	assert_int_equal(plan[6].kind, COPY_GROUP);
}

struct type_case {
	int type;
	bool scene_is_group;
	enum copy_kind child;
};

static void scene_and_group_types(void **state)
{
	(void)state;
	const void *const sources[] = {SRC(1), SRC(2)};
	const bool groups[] = {false, true};
	struct copy_plan plan[2];

	const struct type_case cases[] = {
		{OBS_SOURCE_DUPLICATE_DUP_REFS, false, COPY_REF},
		{OBS_SOURCE_DUPLICATE_DUP_COPY, false, COPY_NEW},
		{OBS_SOURCE_DUPLICATE_DUP_PRIVATE_REFS, false, COPY_REF},
		{OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY, false, COPY_NEW},
		/* a public copy of a group keeps its sources */
		{OBS_SOURCE_DUPLICATE_DUP_REFS, true, COPY_REF},
		{OBS_SOURCE_DUPLICATE_DUP_COPY, true, COPY_REF},
		{OBS_SOURCE_DUPLICATE_DUP_PRIVATE_REFS, true, COPY_REF},
		{OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY, true, COPY_NEW},
	};

	for (size_t i = 0; i < ARRAY_LEN(cases); i++) {
		scene_copies(cases[i].type, cases[i].scene_is_group, sources, groups, 2, plan);
		assert_int_equal(plan[0].kind, cases[i].child);
		/* a group inside is always copied, with the same type */
		assert_int_equal(plan[1].kind, COPY_GROUP);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(type_step),
		cmocka_unit_test(scene_steps),
		cmocka_unit_test(flags_and_filters),
		cmocka_unit_test(source_routes),
		cmocka_unit_test(filter_numbers),
		cmocka_unit_test(filter_names),
		cmocka_unit_test(scene_flags),
		cmocka_unit_test(item_routes),
		cmocka_unit_test(names_and_resize),
		cmocka_unit_test(reuse_step),
		cmocka_unit_test(scene_one_source_three_times),
		cmocka_unit_test(scene_mixed_sources),
		cmocka_unit_test(scene_and_group_types),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
