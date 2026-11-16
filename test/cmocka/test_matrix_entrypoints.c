/*
 * The public matrix calls of the graphics context (gs_matrix_push, _pop,
 * _identity, _transpose, _set, _get, _mul, _rotquat, _rotaa, _translate,
 * _scale, _rotaa4f, _translate3f and _scale3f) through the real libobs
 * entry points, with a fabricated graphics context and no GPU. Pins what
 * test_graphics_no_context and test_graphics_state_stacks don't: with a
 * context, each call works on the top of the context's own matrix stack.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

static struct graphics_subsystem graphics;
static unsigned logs;
static int last_level;
static char last_log[160];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	logs++;
	last_level = level;
	vsnprintf(last_log, sizeof(last_log), format, args);
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, graphics.device);
}

/* A matrix with sixteen different values. */
static void test_matrix(struct matrix4 *m, float base)
{
	float *f = (float *)m;
	for (int i = 0; i < 16; i++)
		f[i] = base + (float)i * 0.25f - (float)(i % 3);
}

static int setup(void **state)
{
	(void)state;
	struct matrix4 id;
	memset(&graphics, 0, sizeof(graphics));
	logs = 0;
	last_log[0] = '\0';
	graphics.device = (gs_device_t *)&graphics;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	/* The stack as graphics_init leaves it: one identity matrix. */
	matrix4_identity(&id);
	da_init(graphics.matrix_stack);
	da_push_back(graphics.matrix_stack, &id);
	graphics.cur_matrix = 0;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_set_log_handler(capture_log, NULL);
	assert_null(gs_get_context());
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(NULL, NULL);
	da_free(graphics.matrix_stack);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void expect_no_context_log(const char *name)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", name);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	logs = 0;
	last_log[0] = '\0';
}

/* Without a context each call does nothing to any stack and logs its own
 * name at debug level. gs_matrix_get leaves its output alone. */
static void no_context(void **state)
{
	(void)state;
	struct matrix4 m, before, got;
	struct quat q;
	struct axisang aa;
	struct vec3 v;
	test_matrix(&m, 3.0f);
	quat_set(&q, 0.2f, -0.4f, 0.1f, 0.88f);
	axisang_set(&aa, 0.3f, 0.5f, -0.8f, 1.25f);
	vec3_set(&v, 1.5f, -2.0f, 0.75f);
	before = graphics.matrix_stack.array[0];

	gs_matrix_push();
	expect_no_context_log("gs_matrix_push");
	gs_matrix_pop();
	expect_no_context_log("gs_matrix_pop");
	gs_matrix_identity();
	expect_no_context_log("gs_matrix_identity");
	gs_matrix_transpose();
	expect_no_context_log("gs_matrix_transpose");
	gs_matrix_set(&m);
	expect_no_context_log("gs_matrix_set");
	got = m;
	gs_matrix_get(&got);
	expect_no_context_log("gs_matrix_get");
	assert_memory_equal(&got, &m, sizeof(got));
	gs_matrix_mul(&m);
	expect_no_context_log("gs_matrix_mul");
	gs_matrix_rotquat(&q);
	expect_no_context_log("gs_matrix_rotquat");
	gs_matrix_rotaa(&aa);
	expect_no_context_log("gs_matrix_rotaa");
	gs_matrix_translate(&v);
	expect_no_context_log("gs_matrix_translate");
	gs_matrix_scale(&v);
	expect_no_context_log("gs_matrix_scale");
	gs_matrix_rotaa4f(0.3f, 0.5f, -0.8f, 1.25f);
	expect_no_context_log("gs_matrix_rotaa4f");
	gs_matrix_translate3f(1.5f, -2.0f, 0.75f);
	expect_no_context_log("gs_matrix_translate3f");
	gs_matrix_scale3f(1.5f, -2.0f, 0.75f);
	expect_no_context_log("gs_matrix_scale3f");

	assert_int_equal(graphics.matrix_stack.num, 1);
	assert_int_equal(graphics.cur_matrix, 0);
	assert_memory_equal(&graphics.matrix_stack.array[0], &before, sizeof(before));
}

/* Push copies the top, set and get work on the new top, and pop goes
 * back to the earlier one, which push and set left alone. */
static void push_set_get_pop(void **state)
{
	(void)state;
	struct matrix4 a, b, got;
	test_matrix(&a, 1.0f);
	test_matrix(&b, -2.0f);
	gs_enter_context(&graphics);

	gs_matrix_set(&a);
	gs_matrix_get(&got);
	assert_memory_equal(&got, &a, sizeof(got));

	for (size_t depth = 1; depth <= 4; depth++) {
		gs_matrix_push();
		assert_int_equal(graphics.cur_matrix, depth);
		assert_int_equal(graphics.matrix_stack.num, depth + 1);
		gs_matrix_get(&got);
		assert_memory_equal(&got, &a, sizeof(got));
	}

	gs_matrix_set(&b);
	gs_matrix_get(&got);
	assert_memory_equal(&got, &b, sizeof(got));
	assert_memory_equal(&graphics.matrix_stack.array[3], &a, sizeof(a));

	gs_matrix_pop();
	assert_int_equal(graphics.cur_matrix, 3);
	assert_int_equal(graphics.matrix_stack.num, 4);
	gs_matrix_get(&got);
	assert_memory_equal(&got, &a, sizeof(got));

	for (size_t depth = 3; depth > 0; depth--)
		gs_matrix_pop();
	assert_int_equal(graphics.cur_matrix, 0);
	assert_int_equal(graphics.matrix_stack.num, 1);
	gs_matrix_get(&got);
	assert_memory_equal(&got, &a, sizeof(got));
	assert_int_equal(logs, 0);
}

/* Pop on the last matrix logs at error level and changes nothing. */
static void pop_last_matrix(void **state)
{
	(void)state;
	struct matrix4 a, got;
	test_matrix(&a, 4.0f);
	gs_enter_context(&graphics);
	gs_matrix_set(&a);

	for (unsigned i = 1; i <= 2; i++) {
		gs_matrix_pop();
		assert_int_equal(logs, i);
		assert_int_equal(last_level, LOG_ERROR);
		assert_string_equal(last_log, "Tried to pop last matrix on stack");
		assert_int_equal(graphics.cur_matrix, 0);
		assert_int_equal(graphics.matrix_stack.num, 1);
		gs_matrix_get(&got);
		assert_memory_equal(&got, &a, sizeof(got));
	}

	/* After a push, one pop works again and logs nothing. */
	gs_matrix_push();
	gs_matrix_pop();
	assert_int_equal(logs, 2);
	assert_int_equal(graphics.cur_matrix, 0);
}

enum matrix_op {
	OP_IDENTITY,
	OP_TRANSPOSE,
	OP_MUL,
	OP_ROTQUAT,
	OP_ROTAA,
	OP_TRANSLATE,
	OP_SCALE,
	OP_ROTAA4F,
	OP_TRANSLATE3F,
	OP_SCALE3F,
	OP_COUNT,
};

/* Each operation changes only the top, as its matrix4 function does, at
 * stack depths 0 to 2. The 3f and 4f forms give the same matrix as their
 * vector forms. */
static void operations_on_top(void **state)
{
	(void)state;
	struct matrix4 start, below, arg, expect, got;
	struct quat q;
	struct axisang aa;
	struct vec3 v;
	test_matrix(&start, 2.0f);
	test_matrix(&below, 7.0f);
	test_matrix(&arg, -1.5f);
	quat_set(&q, 0.2f, -0.4f, 0.1f, 0.88f);
	axisang_set(&aa, 0.3f, 0.5f, -0.8f, 1.25f);
	vec3_set(&v, 1.5f, -2.0f, 0.75f);
	gs_enter_context(&graphics);

	for (size_t depth = 0; depth < 3; depth++) {
		for (int op = 0; op < OP_COUNT; op++) {
			while (graphics.cur_matrix)
				gs_matrix_pop();
			gs_matrix_set(&below);
			for (size_t d = 0; d < depth; d++)
				gs_matrix_push();
			gs_matrix_set(&start);
			expect = start;

			switch (op) {
			case OP_IDENTITY:
				gs_matrix_identity();
				matrix4_identity(&expect);
				break;
			case OP_TRANSPOSE:
				gs_matrix_transpose();
				matrix4_transpose(&expect, &expect);
				break;
			case OP_MUL:
				gs_matrix_mul(&arg);
				matrix4_mul(&expect, &arg, &expect);
				break;
			case OP_ROTQUAT:
				gs_matrix_rotquat(&q);
				matrix4_rotate_i(&expect, &q, &expect);
				break;
			case OP_ROTAA:
				gs_matrix_rotaa(&aa);
				matrix4_rotate_aa_i(&expect, &aa, &expect);
				break;
			case OP_TRANSLATE:
				gs_matrix_translate(&v);
				matrix4_translate3v_i(&expect, &v, &expect);
				break;
			case OP_SCALE:
				gs_matrix_scale(&v);
				matrix4_scale_i(&expect, &v, &expect);
				break;
			case OP_ROTAA4F:
				gs_matrix_rotaa4f(0.3f, 0.5f, -0.8f, 1.25f);
				matrix4_rotate_aa_i(&expect, &aa, &expect);
				break;
			case OP_TRANSLATE3F:
				gs_matrix_translate3f(1.5f, -2.0f, 0.75f);
				matrix4_translate3v_i(&expect, &v, &expect);
				break;
			case OP_SCALE3F:
				gs_matrix_scale3f(1.5f, -2.0f, 0.75f);
				matrix4_scale_i(&expect, &v, &expect);
				break;
			default:
				break;
			}

			assert_int_equal(graphics.cur_matrix, depth);
			assert_int_equal(graphics.matrix_stack.num, depth + 1);
			gs_matrix_get(&got);
			assert_memory_equal(&got, &expect, sizeof(got));
			assert_memory_equal(&graphics.matrix_stack.array[depth], &expect, sizeof(got));
			for (size_t d = 0; d < depth; d++)
				assert_memory_equal(&graphics.matrix_stack.array[d], &below, sizeof(below));
		}
	}
	assert_int_equal(logs, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_context, setup, teardown),
		cmocka_unit_test_setup_teardown(push_set_get_pop, setup, teardown),
		cmocka_unit_test_setup_teardown(pop_last_matrix, setup, teardown),
		cmocka_unit_test_setup_teardown(operations_on_top, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
