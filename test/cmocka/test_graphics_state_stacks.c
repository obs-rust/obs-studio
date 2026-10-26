/*
 * The state stacks of the graphics context (libobs/graphics/
 * graphics-state-stacks.c) on plain darrays, before their port to
 * obs-graphics (obs-rust/obs-studio#2517): the matrix stack (push, pop,
 * the top and each operation on it against the matrix4 functions), the
 * viewport stack, the blend state stack and the steps of
 * gs_reset_blend_state. No graphics context is needed.
 *
 * test_graphics_state_stacks_rust builds the same cases with
 * GS_STATE_STACKS_TEST_RUST: without graphics-state-stacks.c, against the
 * Rust shims of obs-graphics.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>

#ifndef GS_STATE_STACKS_TEST_RUST
#include "graphics/graphics-state-stacks.c.h"
#else
#include "graphics/graphics-state-stacks.h"
#endif

/* A matrix with sixteen different values. */
static void test_matrix(struct matrix4 *m, float base)
{
	float *f = (float *)m;
	for (int i = 0; i < 16; i++)
		f[i] = base + (float)i * 0.25f - (float)(i % 3);
}

struct mstack {
	DARRAY(struct matrix4) da;
	size_t cur;
};

/* A stack with the identity, as graphics_init makes it. */
static void mstack_init(struct mstack *s)
{
	struct matrix4 id;
	matrix4_identity(&id);
	da_init(s->da);
	da_push_back(s->da, &id);
	s->cur = 0;
}

static void matrix_push_pop(void **state)
{
	(void)state;
	struct mstack s;
	mstack_init(&s);

	struct matrix4 a, b;
	test_matrix(&a, 1.0f);
	gs_matrix_stack_set(&s.da.da, s.cur, &a);

	/* Push copies the top. */
	for (size_t depth = 1; depth <= 5; depth++) {
		gs_matrix_stack_push(&s.da.da, &s.cur);
		assert_int_equal(s.cur, depth);
		assert_int_equal(s.da.num, depth + 1);
		assert_memory_equal(&s.da.array[s.cur], &s.da.array[s.cur - 1], sizeof(struct matrix4));
	}

	/* An operation changes only the top; pop brings back the one below. */
	test_matrix(&b, -3.0f);
	gs_matrix_stack_set(&s.da.da, s.cur, &b);
	assert_memory_equal(&s.da.array[s.cur - 1], &a, sizeof(a));
	for (size_t depth = 5; depth > 0; depth--) {
		assert_true(gs_matrix_stack_pop(&s.da.da, &s.cur));
		assert_int_equal(s.cur, depth - 1);
		assert_int_equal(s.da.num, depth);
	}
	assert_memory_equal(&s.da.array[0], &a, sizeof(a));

	/* The last matrix stays: pop says so and changes nothing. */
	assert_false(gs_matrix_stack_pop(&s.da.da, &s.cur));
	assert_int_equal(s.cur, 0);
	assert_int_equal(s.da.num, 1);
	da_free(s.da);
}

/* Every operation on the top equals the matrix4 function on the same
 * matrix, at the bottom and higher up. */
static void matrix_operations(void **state)
{
	(void)state;
	struct matrix4 start, arg, expect, got;
	struct quat q;
	struct axisang aa;
	struct vec3 v;
	test_matrix(&start, 2.0f);
	test_matrix(&arg, -1.5f);
	quat_set(&q, 0.2f, -0.4f, 0.1f, 0.88f);
	axisang_set(&aa, 0.3f, 0.5f, -0.8f, 1.25f);
	vec3_set(&v, 1.5f, -2.0f, 0.75f);

	for (int depth = 0; depth < 3; depth++) {
		for (int op = 0; op < 9; op++) {
			struct mstack s;
			mstack_init(&s);
			for (int d = 0; d < depth; d++)
				gs_matrix_stack_push(&s.da.da, &s.cur);
			gs_matrix_stack_set(&s.da.da, s.cur, &start);
			expect = start;

			switch (op) {
			case 0:
				gs_matrix_stack_identity(&s.da.da, s.cur);
				matrix4_identity(&expect);
				break;
			case 1:
				gs_matrix_stack_transpose(&s.da.da, s.cur);
				matrix4_transpose(&expect, &expect);
				break;
			case 2:
				gs_matrix_stack_set(&s.da.da, s.cur, &arg);
				expect = arg;
				break;
			case 3:
				gs_matrix_stack_mul(&s.da.da, s.cur, &arg);
				matrix4_mul(&expect, &arg, &expect);
				break;
			case 4:
				gs_matrix_stack_rotquat(&s.da.da, s.cur, &q);
				matrix4_rotate_i(&expect, &q, &expect);
				break;
			case 5:
				gs_matrix_stack_rotaa(&s.da.da, s.cur, &aa);
				matrix4_rotate_aa_i(&expect, &aa, &expect);
				break;
			case 6:
				gs_matrix_stack_translate(&s.da.da, s.cur, &v);
				matrix4_translate3v_i(&expect, &v, &expect);
				break;
			case 7:
				gs_matrix_stack_scale(&s.da.da, s.cur, &v);
				matrix4_scale_i(&expect, &v, &expect);
				break;
			default:
				break; /* get only */
			}

			gs_matrix_stack_get(&s.da.da, s.cur, &got);
			assert_memory_equal(&got, &expect, sizeof(got));
			assert_memory_equal(&s.da.array[s.cur], &expect, sizeof(got));
			da_free(s.da);
		}
	}
}

static void viewport_stack(void **state)
{
	(void)state;
	DARRAY(struct gs_rect) da;
	da_init(da);
	struct gs_rect r;

	assert_false(gs_viewport_stack_pop(&da.da, &r));

	for (int i = 0; i < 4; i++) {
		struct gs_rect in = {i, i * 2, 100 + i, 50 + i};
		gs_viewport_stack_push(&da.da, &in);
	}
	assert_int_equal(da.num, 4);
	for (int i = 3; i >= 0; i--) {
		memset(&r, 0x5A, sizeof(r));
		assert_true(gs_viewport_stack_pop(&da.da, &r));
		assert_int_equal(r.x, i);
		assert_int_equal(r.y, i * 2);
		assert_int_equal(r.cx, 100 + i);
		assert_int_equal(r.cy, 50 + i);
	}
	assert_int_equal(da.num, 0);
	struct gs_rect untouched;
	memset(&untouched, 0x5A, sizeof(untouched));
	r = untouched;
	assert_false(gs_viewport_stack_pop(&da.da, &r));
	assert_memory_equal(&r, &untouched, sizeof(r));
	da_free(da);
}

static void blend_state_stack(void **state)
{
	(void)state;
	DARRAY(struct blend_state) da;
	da_init(da);
	struct blend_state st;

	assert_false(gs_blend_state_stack_pop(&da.da, &st));

	for (int i = 0; i < 3; i++) {
		struct blend_state in = {
			.enabled = i % 2 == 0,
			.src_c = (enum gs_blend_type)i,
			.dest_c = (enum gs_blend_type)(i + 1),
			.src_a = (enum gs_blend_type)(i + 2),
			.dest_a = (enum gs_blend_type)(i + 3),
			.op = (enum gs_blend_op_type)(i % 5),
		};
		gs_blend_state_stack_push(&da.da, &in);
	}
	for (int i = 2; i >= 0; i--) {
		assert_true(gs_blend_state_stack_pop(&da.da, &st));
		assert_int_equal(st.enabled, i % 2 == 0);
		assert_int_equal(st.src_c, i);
		assert_int_equal(st.dest_c, i + 1);
		assert_int_equal(st.src_a, i + 2);
		assert_int_equal(st.dest_a, i + 3);
		assert_int_equal(st.op, i % 5);
	}
	assert_false(gs_blend_state_stack_pop(&da.da, &st));
	da_free(da);
}

static const struct blend_state blend_default = {
	.enabled = true,
	.src_c = GS_BLEND_SRCALPHA,
	.dest_c = GS_BLEND_INVSRCALPHA,
	.src_a = GS_BLEND_ONE,
	.dest_a = GS_BLEND_INVSRCALPHA,
	.op = GS_BLEND_OP_ADD,
};

static void blend_reset_steps(void **state)
{
	(void)state;
	struct gs_blend_reset steps;
	struct blend_state st;

	/* The default state: nothing to do. */
	steps = gs_blend_state_reset_steps(&blend_default);
	assert_false(steps.enable);
	assert_false(steps.function);
	assert_false(steps.op);

	/* Blending off: enable it. */
	st = blend_default;
	st.enabled = false;
	steps = gs_blend_state_reset_steps(&st);
	assert_true(steps.enable);
	assert_false(steps.function);
	assert_false(steps.op);

	/* Any function field off its default: the function and the op. */
	for (int field = 0; field < 4; field++) {
		st = blend_default;
		enum gs_blend_type *f[4] = {&st.src_c, &st.dest_c, &st.src_a, &st.dest_a};
		*f[field] = GS_BLEND_ZERO;
		steps = gs_blend_state_reset_steps(&st);
		assert_false(steps.enable);
		assert_true(steps.function);
		assert_true(steps.op);
	}

	/* A blend mode such as Subtract: function and op both off. */
	st = blend_default;
	st.src_c = st.dest_c = st.src_a = st.dest_a = GS_BLEND_ONE;
	st.op = GS_BLEND_OP_REVERSE_SUBTRACT;
	steps = gs_blend_state_reset_steps(&st);
	assert_true(steps.function);
	assert_true(steps.op);

	/* The default function with another op: the op is reset too, as the
	 * reset sets the whole state to its default (#2519). */
	for (int op = GS_BLEND_OP_SUBTRACT; op <= GS_BLEND_OP_MAX; op++) {
		st = blend_default;
		st.op = (enum gs_blend_op_type)op;
		steps = gs_blend_state_reset_steps(&st);
		assert_false(steps.enable);
		assert_false(steps.function);
		assert_true(steps.op);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(matrix_push_pop),   cmocka_unit_test(matrix_operations),
		cmocka_unit_test(viewport_stack),    cmocka_unit_test(blend_state_stack),
		cmocka_unit_test(blend_reset_steps),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
