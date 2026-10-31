#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/graphics-immediate.h"

/*
 * Characterization of the decisions of the immediate-mode vertex builder of
 * libobs/graphics/graphics.c, moved to libobs/graphics/graphics-immediate.c
 * before their Rust port (#3132):
 * - the push gates of gs_vertex3v(), gs_normal3v(), gs_color() and
 *   gs_texcoord2v(): gs_immediate_unit_ok() and gs_immediate_push_ok();
 * - gs_render_stop(): gs_immediate_mismatch() and gs_immediate_cut() for
 *   each of the normal, colour and texture 0 arrays;
 * - gs_render_save(): gs_immediate_save_cut() for each array its buffer
 *   takes, and gs_immediate_save_free_from().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so test_graphics_immediate_steps
 * compiles the C source into itself and calls only them, through their
 * header. test_graphics_immediate_steps_rust (GS_IMMEDIATE_TEST_RUST) runs
 * the same cases against the Rust shims of the hybrid build, and adds the
 * port's intentional differences, where the C cases below differ: a unit
 * outside 0-15 or above 0 in immediate mode is refused, a save is cut to a
 * shorter array, and a save frees the texture arrays it does not take.
 * stop_count() and save_count() below ask the functions as graphics.c does;
 * graphics.c keeps the arrays, every read, push and free, the vertex buffer
 * calls and the logging.
 */

/* gs_render_stop()'s draw count, with the number of error logs. */
static size_t stop_count(size_t verts, size_t norms, size_t colors, size_t tex0, int *logs)
{
	size_t num = verts;
	const size_t counts[3] = {norms, colors, tex0};

	*logs = 0;
	if (!num)
		return 0;
	for (size_t i = 0; i < 3; i++) {
		if (gs_immediate_mismatch(counts[i], verts)) {
			(*logs)++;
			num = gs_immediate_cut(num, counts[i]);
		}
	}
	return num;
}

/* gs_render_save()'s vertex count for a buffer with num_tex texture arrays
 * of tex vertices each. */
static size_t save_count(size_t verts, size_t norms, size_t colors, size_t num_tex, size_t tex)
{
	size_t num = verts;

	num = gs_immediate_save_cut(num, norms);
	num = gs_immediate_save_cut(num, colors);
	for (size_t i = 0; i < num_tex; i++)
		num = gs_immediate_save_cut(num, tex);
	return num;
}

/* ------------------------------------------------------------------------ */

static void matching_counts_draw_every_vertex(void **state)
{
	int logs;

	(void)state;

	assert_int_equal(stop_count(1, 0, 0, 0, &logs), 1);
	assert_int_equal(logs, 0);
	assert_int_equal(stop_count(4, 4, 4, 4, &logs), 4);
	assert_int_equal(logs, 0);
	assert_int_equal(stop_count(4, 0, 4, 0, &logs), 4);
	assert_int_equal(logs, 0);
	assert_int_equal(stop_count(GS_IMMEDIATE_COUNT, GS_IMMEDIATE_COUNT, GS_IMMEDIATE_COUNT, GS_IMMEDIATE_COUNT,
				    &logs),
			 GS_IMMEDIATE_COUNT);
	assert_int_equal(logs, 0);
}

static void a_shorter_array_cuts_the_draw(void **state)
{
	int logs;

	(void)state;

	assert_true(gs_immediate_mismatch(3, 4));
	assert_int_equal(gs_immediate_cut(4, 3), 3);
	assert_int_equal(gs_immediate_cut(3, 3), 3);
	assert_int_equal(gs_immediate_cut(0, 3), 0);

	/* Each of normals, colours and texture 0 shorter than the vertices:
	 * the draw is cut to it and its error logs. */
	assert_int_equal(stop_count(4, 3, 4, 4, &logs), 3);
	assert_int_equal(logs, 1);
	assert_int_equal(stop_count(4, 4, 2, 4, &logs), 2);
	assert_int_equal(logs, 1);
	assert_int_equal(stop_count(4, 4, 4, 1, &logs), 1);
	assert_int_equal(logs, 1);

	/* All three: the smallest, and three errors. */
	assert_int_equal(stop_count(5, 4, 3, 2, &logs), 2);
	assert_int_equal(logs, 3);
	assert_int_equal(stop_count(5, 2, 3, 4, &logs), 2);
	assert_int_equal(logs, 3);
}

static void an_empty_array_never_differs(void **state)
{
	int logs;

	(void)state;

	assert_false(gs_immediate_mismatch(0, 0));
	assert_false(gs_immediate_mismatch(0, 1));
	assert_false(gs_immediate_mismatch(0, GS_IMMEDIATE_COUNT));
	assert_false(gs_immediate_mismatch(4, 4));
	assert_int_equal(stop_count(5, 0, 3, 0, &logs), 3);
	assert_int_equal(logs, 1);
}

static void a_longer_array_logs_without_a_cut(void **state)
{
	int logs;

	(void)state;

	/* A longer array logs its error too, and the vertices are drawn
	 * (characterized, not endorsed: nothing is wrong with the draw). */
	assert_true(gs_immediate_mismatch(6, 4));
	assert_int_equal(gs_immediate_cut(4, 6), 4);
	assert_int_equal(stop_count(4, 6, 4, 4, &logs), 4);
	assert_int_equal(logs, 1);
	assert_int_equal(stop_count(4, 9, 2, 6, &logs), 2);
	assert_int_equal(logs, 3);
}

static void the_immediate_cap(void **state)
{
	(void)state;

	/* In immediate mode an array takes vertices up to GS_IMMEDIATE_COUNT. */
	assert_true(gs_immediate_push_ok(true, 0));
	assert_true(gs_immediate_push_ok(true, 1));
	assert_true(gs_immediate_push_ok(true, GS_IMMEDIATE_COUNT - 1));
	assert_false(gs_immediate_push_ok(true, GS_IMMEDIATE_COUNT));

	/* Otherwise there is no cap. */
	assert_true(gs_immediate_push_ok(false, 0));
	assert_true(gs_immediate_push_ok(false, GS_IMMEDIATE_COUNT - 1));
	assert_true(gs_immediate_push_ok(false, GS_IMMEDIATE_COUNT));
	assert_true(gs_immediate_push_ok(false, GS_IMMEDIATE_COUNT + 1));
	assert_true(gs_immediate_push_ok(false, 100000));
	assert_true(gs_immediate_push_ok(false, SIZE_MAX));
}

static void the_backed_texture_units(void **state)
{
	(void)state;

	/* Texture unit 0 in both modes, and every unit outside immediate
	 * mode. */
	assert_true(gs_immediate_unit_ok(true, 0));
	for (int unit = 0; unit < GS_IMMEDIATE_TEX_ARRAYS; unit++)
		assert_true(gs_immediate_unit_ok(false, unit));
}

static void a_full_save_keeps_its_count(void **state)
{
	(void)state;

	/* Arrays as long as the vertices, longer or empty: the buffer has
	 * every vertex. */
	assert_int_equal(gs_immediate_save_cut(4, 0), 4);
	assert_int_equal(gs_immediate_save_cut(4, 4), 4);
	assert_int_equal(gs_immediate_save_cut(4, 9), 4);
	assert_int_equal(save_count(1, 1, 1, 1, 1), 1);
	assert_int_equal(save_count(4, 4, 4, 1, 4), 4);
	assert_int_equal(save_count(7, 0, 0, 0, 0), 7);
	assert_int_equal(save_count(3, 9, 0, GS_IMMEDIATE_TEX_ARRAYS, 3), 3);
	assert_int_equal(save_count(GS_IMMEDIATE_COUNT, GS_IMMEDIATE_COUNT, GS_IMMEDIATE_COUNT, GS_IMMEDIATE_TEX_ARRAYS,
				    GS_IMMEDIATE_COUNT),
			 GS_IMMEDIATE_COUNT);
}

static void a_save_taking_every_array_frees_none(void **state)
{
	(void)state;

	assert_int_equal(gs_immediate_save_free_from(GS_IMMEDIATE_TEX_ARRAYS), GS_IMMEDIATE_TEX_ARRAYS);

	/* The arrays freed start at or after the ones the buffer takes, and
	 * stay inside the 16. */
	for (size_t num_tex = 0; num_tex <= GS_IMMEDIATE_TEX_ARRAYS; num_tex++) {
		size_t from = gs_immediate_save_free_from(num_tex);
		assert_true(from >= num_tex);
		assert_true(from <= GS_IMMEDIATE_TEX_ARRAYS);
	}
}

#ifdef GS_IMMEDIATE_TEST_RUST

/* ------------------------------------------------------------------------ */
/* The Rust port's intentional differences */

static void the_port_refuses_unbacked_units(void **state)
{
	(void)state;

	assert_false(gs_immediate_unit_ok(false, -1));
	assert_false(gs_immediate_unit_ok(false, GS_IMMEDIATE_TEX_ARRAYS));
	assert_false(gs_immediate_unit_ok(true, -1));
	assert_false(gs_immediate_unit_ok(true, GS_IMMEDIATE_TEX_ARRAYS));
	for (int unit = 1; unit < GS_IMMEDIATE_TEX_ARRAYS; unit++)
		assert_false(gs_immediate_unit_ok(true, unit));
}

static void the_port_cuts_a_save_to_a_shorter_array(void **state)
{
	(void)state;

	assert_int_equal(gs_immediate_save_cut(4, 3), 3);
	assert_int_equal(save_count(4, 3, 4, 0, 0), 3);
	assert_int_equal(save_count(4, 4, 2, 0, 0), 2);
	assert_int_equal(save_count(4, 4, 4, 2, 1), 1);
}

static void the_port_frees_the_arrays_a_save_leaves(void **state)
{
	(void)state;

	for (size_t num_tex = 0; num_tex <= GS_IMMEDIATE_TEX_ARRAYS; num_tex++)
		assert_int_equal(gs_immediate_save_free_from(num_tex), num_tex);
}

#else

/* ------------------------------------------------------------------------ */
/* The C, where the port differs on purpose */

static void the_c_takes_any_unit(void **state)
{
	(void)state;

	/* Characterized, not endorsed: graphics.c then indexes texverts with
	 * it, or pushes to an array the immediate buffer never draws. */
	assert_true(gs_immediate_unit_ok(false, -1));
	assert_true(gs_immediate_unit_ok(false, GS_IMMEDIATE_TEX_ARRAYS));
	assert_true(gs_immediate_unit_ok(true, 1));
}

static void the_c_saves_every_vertex(void **state)
{
	(void)state;

	/* Characterized, not endorsed: the device reads past the shorter
	 * array. */
	assert_int_equal(save_count(4, 3, 4, 0, 0), 4);
	assert_int_equal(save_count(4, 4, 4, 2, 1), 4);
}

static void the_c_frees_no_texture_array(void **state)
{
	(void)state;

	/* Characterized, not endorsed: the arrays after the first empty one
	 * are dropped without a free. */
	assert_int_equal(gs_immediate_save_free_from(0), GS_IMMEDIATE_TEX_ARRAYS);
	assert_int_equal(gs_immediate_save_free_from(3), GS_IMMEDIATE_TEX_ARRAYS);
}

#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(matching_counts_draw_every_vertex),
		cmocka_unit_test(a_shorter_array_cuts_the_draw),
		cmocka_unit_test(an_empty_array_never_differs),
		cmocka_unit_test(a_longer_array_logs_without_a_cut),
		cmocka_unit_test(the_immediate_cap),
		cmocka_unit_test(the_backed_texture_units),
		cmocka_unit_test(a_full_save_keeps_its_count),
		cmocka_unit_test(a_save_taking_every_array_frees_none),
#ifdef GS_IMMEDIATE_TEST_RUST
		cmocka_unit_test(the_port_refuses_unbacked_units),
		cmocka_unit_test(the_port_cuts_a_save_to_a_shorter_array),
		cmocka_unit_test(the_port_frees_the_arrays_a_save_leaves),
#else
		cmocka_unit_test(the_c_takes_any_unit),
		cmocka_unit_test(the_c_saves_every_vertex),
		cmocka_unit_test(the_c_frees_no_texture_array),
#endif
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
