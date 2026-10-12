/* Regression test for obsproject/obs-studio#4245 (reported by lindenkron):
 * the scale a bounding box gives a scene item must stay finite when the
 * item's source has no size.
 *
 * obs_bounds_fit_scale() is the scale half of the scene item transform
 * (calculate_bounds_data() in obs-scene.c). It divided the bounding box by
 * the source's size, so a source with no size (an empty group, or a source
 * that has not loaded yet) got an infinite or NaN scale. For a group, every
 * item added to it is then mapped through that transform and becomes NaN
 * too (test_group_empty_bounds covers that with a real scene).
 *
 * This test needs no display and no libobs core. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>

#include <obs-scene-bounds.h>

static const enum obs_bounds_type all_types[] = {
	OBS_BOUNDS_STRETCH,        OBS_BOUNDS_SCALE_INNER,     OBS_BOUNDS_SCALE_OUTER,
	OBS_BOUNDS_SCALE_TO_WIDTH, OBS_BOUNDS_SCALE_TO_HEIGHT, OBS_BOUNDS_MAX_ONLY,
};

#define NUM_TYPES (sizeof(all_types) / sizeof(all_types[0]))

static const char *type_name(enum obs_bounds_type type)
{
	switch (type) {
	case OBS_BOUNDS_NONE:
		return "none";
	case OBS_BOUNDS_STRETCH:
		return "stretch";
	case OBS_BOUNDS_SCALE_INNER:
		return "scale inner";
	case OBS_BOUNDS_SCALE_OUTER:
		return "scale outer";
	case OBS_BOUNDS_SCALE_TO_WIDTH:
		return "scale to width";
	case OBS_BOUNDS_SCALE_TO_HEIGHT:
		return "scale to height";
	case OBS_BOUNDS_MAX_ONLY:
		return "max only";
	}
	return "?";
}

static struct vec2 fit(enum obs_bounds_type type, float bounds_x, float bounds_y, uint32_t cx, uint32_t cy,
		       float scale_x, float scale_y)
{
	struct vec2 bounds, scale;
	vec2_set(&bounds, bounds_x, bounds_y);
	vec2_set(&scale, scale_x, scale_y);
	obs_bounds_fit_scale(type, &bounds, cx, cy, &scale);
	return scale;
}

static bool finite_nonzero(const struct vec2 *v)
{
	return isfinite(v->x) && isfinite(v->y) && v->x != 0.0f && v->y != 0.0f;
}

/* A source with no size (an empty group) keeps its scale with every bounds
 * type: there is nothing to fit. Before the fix every type but "max only"
 * gave an infinite or NaN scale. */
static void empty_source_keeps_scale(void **state)
{
	(void)state;
	int failures = 0;

	for (size_t i = 0; i < NUM_TYPES; i++) {
		struct vec2 s = fit(all_types[i], 800.0f, 600.0f, 0, 0, 1.0f, 1.0f);
		if (s.x != 1.0f || s.y != 1.0f) {
			fprintf(stderr, "%s: a 0x0 source into 800x600 gives scale (%g, %g), expected (1, 1)\n",
				type_name(all_types[i]), s.x, s.y);
			failures++;
		}
	}

	if (failures) {
		fprintf(stderr, "%d of %zu bounds types give a 0x0 source a non-finite scale\n", failures, NUM_TYPES);
		fail();
	}
}

/* Flips survive: a flipped empty group keeps its flip. */
static void empty_source_keeps_flip(void **state)
{
	(void)state;
	int failures = 0;

	for (size_t i = 0; i < NUM_TYPES; i++) {
		struct vec2 s = fit(all_types[i], 800.0f, 600.0f, 0, 0, -1.0f, 2.0f);
		if (s.x != -1.0f || s.y != 2.0f) {
			fprintf(stderr, "%s: a flipped 0x0 source gives scale (%g, %g), expected (-1, 2)\n",
				type_name(all_types[i]), s.x, s.y);
			failures++;
		}
	}

	if (failures)
		fail();
}

/* A source with a width but no height, or a height but no width (a group
 * whose only item has no width), gets a finite, non-zero scale with every
 * bounds type. */
static void zero_width_or_height_stays_finite(void **state)
{
	(void)state;
	int failures = 0;
	const uint32_t sizes[][2] = {{0, 100}, {200, 0}};

	for (size_t j = 0; j < 2; j++) {
		for (size_t i = 0; i < NUM_TYPES; i++) {
			struct vec2 s = fit(all_types[i], 800.0f, 600.0f, sizes[j][0], sizes[j][1], 1.0f, 1.0f);
			if (!finite_nonzero(&s)) {
				fprintf(stderr, "%s: a %ux%u source into 800x600 gives scale (%g, %g)\n",
					type_name(all_types[i]), sizes[j][0], sizes[j][1], s.x, s.y);
				failures++;
			}
		}
	}

	if (failures)
		fail();
}

/* A base scale of 0 (the item scaled to nothing) must not turn into NaN
 * either. */
static void zero_scale_stays_finite(void **state)
{
	(void)state;
	int failures = 0;

	for (size_t i = 0; i < NUM_TYPES; i++) {
		struct vec2 s = fit(all_types[i], 800.0f, 600.0f, 200, 100, 0.0f, 0.0f);
		if (!isfinite(s.x) || !isfinite(s.y)) {
			fprintf(stderr, "%s: a 200x100 source at scale 0 gives scale (%g, %g)\n",
				type_name(all_types[i]), s.x, s.y);
			failures++;
		}
	}

	if (failures)
		fail();
}

static void expect_scale(enum obs_bounds_type type, uint32_t cx, uint32_t cy, float scale_x, float scale_y,
			 float want_x, float want_y, int *failures)
{
	struct vec2 s = fit(type, 800.0f, 600.0f, cx, cy, scale_x, scale_y);
	if (fabsf(s.x - want_x) > 1e-5f || fabsf(s.y - want_y) > 1e-5f) {
		fprintf(stderr, "%s: a %ux%u source at scale (%g, %g) into 800x600 gives (%g, %g), expected (%g, %g)\n",
			type_name(type), cx, cy, scale_x, scale_y, s.x, s.y, want_x, want_y);
		(*failures)++;
	}
}

/* Controls: sources with a size fit the box as before. */
static void sized_source_fits(void **state)
{
	(void)state;
	int failures = 0;

	/* 200x100 into 800x600: 4x to the width, 6x to the height. */
	expect_scale(OBS_BOUNDS_STRETCH, 200, 100, 1.0f, 1.0f, 4.0f, 6.0f, &failures);
	expect_scale(OBS_BOUNDS_SCALE_INNER, 200, 100, 1.0f, 1.0f, 4.0f, 4.0f, &failures);
	expect_scale(OBS_BOUNDS_SCALE_OUTER, 200, 100, 1.0f, 1.0f, 6.0f, 6.0f, &failures);
	expect_scale(OBS_BOUNDS_SCALE_TO_WIDTH, 200, 100, 1.0f, 1.0f, 4.0f, 4.0f, &failures);
	expect_scale(OBS_BOUNDS_SCALE_TO_HEIGHT, 200, 100, 1.0f, 1.0f, 6.0f, 6.0f, &failures);
	expect_scale(OBS_BOUNDS_MAX_ONLY, 200, 100, 1.0f, 1.0f, 1.0f, 1.0f, &failures);

	/* The base scale counts, except for stretch. */
	expect_scale(OBS_BOUNDS_SCALE_INNER, 200, 100, 2.0f, 2.0f, 4.0f, 4.0f, &failures);
	expect_scale(OBS_BOUNDS_STRETCH, 200, 100, 2.0f, 3.0f, 4.0f, 6.0f, &failures);

	/* Too big for "max only": scaled down like "scale inner". */
	expect_scale(OBS_BOUNDS_MAX_ONLY, 2000, 100, 1.0f, 1.0f, 0.4f, 0.4f, &failures);

	/* Flips are kept. */
	expect_scale(OBS_BOUNDS_SCALE_INNER, 200, 100, -1.0f, 1.0f, -4.0f, 4.0f, &failures);
	expect_scale(OBS_BOUNDS_STRETCH, 200, 100, 1.0f, -1.0f, 4.0f, -6.0f, &failures);

	/* One axis empty: the other axis can still be fitted. */
	expect_scale(OBS_BOUNDS_SCALE_INNER, 0, 100, 1.0f, 1.0f, 6.0f, 6.0f, &failures);
	expect_scale(OBS_BOUNDS_SCALE_TO_WIDTH, 200, 0, 1.0f, 1.0f, 4.0f, 4.0f, &failures);
	expect_scale(OBS_BOUNDS_SCALE_TO_HEIGHT, 0, 100, 1.0f, 1.0f, 6.0f, 6.0f, &failures);

	if (failures)
		fail();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(empty_source_keeps_scale),
		cmocka_unit_test(empty_source_keeps_flip),
		cmocka_unit_test(zero_width_or_height_stays_finite),
		cmocka_unit_test(zero_scale_stays_finite),
		cmocka_unit_test(sized_source_fits),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
