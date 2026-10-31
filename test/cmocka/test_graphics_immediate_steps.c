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
 * - gs_render_stop(): gs_immediate_draw(), the vertices to draw and which
 *   count differs;
 * - gs_render_save(): gs_immediate_save(), the buffer's vertex count and
 *   texture arrays.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. graphics.c keeps the
 * arrays, every push, free and vertex buffer call, and the logging.
 *
 * Not covered: a texture unit outside 0-15 or above 0 in immediate mode, a
 * save with a normal, colour or texture array shorter than the vertices, and
 * which texture arrays the save leaves to free. The C reads and writes past
 * the arrays, or leaks them, there; the Rust port changes them on purpose,
 * and its parity test lists them.
 */

static struct gs_immediate_save save(size_t verts, size_t norms, size_t colors, size_t filled)
{
	size_t tex[GS_IMMEDIATE_TEX_ARRAYS] = {0};

	for (size_t i = 0; i < filled; i++)
		tex[i] = verts;
	return gs_immediate_save(verts, norms, colors, tex);
}

static void check_draw(struct gs_immediate_draw draw, size_t num, bool norms, bool colors, bool tex)
{
	assert_int_equal(draw.num, num);
	assert_int_equal(draw.norms_mismatch, norms);
	assert_int_equal(draw.colors_mismatch, colors);
	assert_int_equal(draw.tex_mismatch, tex);
}

/* ------------------------------------------------------------------------ */

static void an_empty_start_and_stop(void **state)
{
	(void)state;

	/* No vertex: nothing to draw, in either mode (graphics.c frees the
	 * arrays or keeps the immediate buffer). */
	check_draw(gs_immediate_draw(0, 0, 0, 0), 0, false, false, false);

	/* Other arrays without a vertex are not looked at: no error logs
	 * (characterized, not endorsed). */
	check_draw(gs_immediate_draw(0, 3, 2, 1), 0, false, false, false);

	/* No vertex: no buffer. */
	struct gs_immediate_save s = save(0, 0, 0, 0);
	assert_int_equal(s.num, 0);
	assert_int_equal(s.num_tex, 0);

	size_t tex[GS_IMMEDIATE_TEX_ARRAYS] = {1, 1};
	s = gs_immediate_save(0, 3, 2, tex);
	assert_int_equal(s.num, 0);
	assert_int_equal(s.num_tex, 0);
}

static void matching_counts_draw_every_vertex(void **state)
{
	(void)state;

	check_draw(gs_immediate_draw(1, 0, 0, 0), 1, false, false, false);
	check_draw(gs_immediate_draw(4, 4, 4, 4), 4, false, false, false);
	check_draw(gs_immediate_draw(4, 0, 4, 0), 4, false, false, false);
	check_draw(gs_immediate_draw(GS_IMMEDIATE_COUNT, GS_IMMEDIATE_COUNT, GS_IMMEDIATE_COUNT, GS_IMMEDIATE_COUNT),
		   GS_IMMEDIATE_COUNT, false, false, false);

	struct gs_immediate_save s = save(1, 1, 1, 1);
	assert_int_equal(s.num, 1);
	assert_int_equal(s.num_tex, 1);
	s = save(4, 4, 4, 1);
	assert_int_equal(s.num, 4);
	assert_int_equal(s.num_tex, 1);
	s = save(7, 0, 0, 0);
	assert_int_equal(s.num, 7);
	assert_int_equal(s.num_tex, 0);
}

static void a_shorter_array_cuts_the_draw(void **state)
{
	(void)state;

	/* Each of normals, colours and texture 0 shorter than the vertices:
	 * the draw is cut to it and its error logs. */
	check_draw(gs_immediate_draw(4, 3, 4, 4), 3, true, false, false);
	check_draw(gs_immediate_draw(4, 4, 2, 4), 2, false, true, false);
	check_draw(gs_immediate_draw(4, 4, 4, 1), 1, false, false, true);

	/* All three: the smallest, and three errors. */
	check_draw(gs_immediate_draw(5, 4, 3, 2), 2, true, true, true);
	check_draw(gs_immediate_draw(5, 2, 3, 4), 2, true, true, true);

	/* An empty array is not compared. */
	check_draw(gs_immediate_draw(5, 0, 3, 0), 3, false, true, false);
}

static void a_longer_array_logs_without_a_cut(void **state)
{
	(void)state;

	/* A longer array logs its error too, and the vertices are drawn
	 * (characterized, not endorsed: nothing is wrong with the draw). */
	check_draw(gs_immediate_draw(4, 6, 4, 4), 4, true, false, false);
	check_draw(gs_immediate_draw(4, 4, 5, 4), 4, false, true, false);
	check_draw(gs_immediate_draw(4, 4, 4, 9), 4, false, false, true);
	check_draw(gs_immediate_draw(4, 9, 2, 6), 2, true, true, true);
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

static void the_texture_array_count(void **state)
{
	(void)state;

	/* The leading non-empty arrays. */
	struct gs_immediate_save s = save(3, 3, 3, 3);
	assert_int_equal(s.num, 3);
	assert_int_equal(s.num_tex, 3);

	s = save(3, 0, 0, GS_IMMEDIATE_TEX_ARRAYS);
	assert_int_equal(s.num_tex, GS_IMMEDIATE_TEX_ARRAYS);

	/* Array 0 empty and array 1 filled: no texture array
	 * (characterized, not endorsed: array 1 is set and not drawn). */
	size_t tex[GS_IMMEDIATE_TEX_ARRAYS] = {0};
	tex[1] = 3;
	s = gs_immediate_save(3, 3, 3, tex);
	assert_int_equal(s.num, 3);
	assert_int_equal(s.num_tex, 0);

	/* A gap after array 1: arrays 0 and 1. */
	tex[0] = 3;
	tex[3] = 3;
	s = gs_immediate_save(3, 0, 3, tex);
	assert_int_equal(s.num_tex, 2);

	/* Only the counts' being non-zero matters, not their values. */
	tex[2] = 1;
	s = gs_immediate_save(3, 0, 3, tex);
	assert_int_equal(s.num_tex, 4);
}

static void the_save_frees_no_array_it_takes(void **state)
{
	(void)state;

	/* The arrays graphics.c frees start at or after the ones the buffer
	 * takes, and stay inside the 16. */
	for (size_t filled = 0; filled <= GS_IMMEDIATE_TEX_ARRAYS; filled++) {
		struct gs_immediate_save s = save(2, 2, 2, filled);
		assert_int_equal(s.num_tex, filled);
		assert_true(s.free_from >= s.num_tex);
		assert_true(s.free_from <= GS_IMMEDIATE_TEX_ARRAYS);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(an_empty_start_and_stop),
		cmocka_unit_test(matching_counts_draw_every_vertex),
		cmocka_unit_test(a_shorter_array_cuts_the_draw),
		cmocka_unit_test(a_longer_array_logs_without_a_cut),
		cmocka_unit_test(the_immediate_cap),
		cmocka_unit_test(the_backed_texture_units),
		cmocka_unit_test(the_texture_array_count),
		cmocka_unit_test(the_save_frees_no_array_it_takes),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
