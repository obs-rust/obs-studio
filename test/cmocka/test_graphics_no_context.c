/*
 * The libobs/graphics entry points without a graphics context on the
 * thread (obs-rust/obs-studio#2518): each one returns and logs
 * "<name>: called while not in a graphics context" (LOG_DEBUG, through
 * gs_valid) instead of reading the missing context. gs_matrix_scale3f
 * read thread_graphics directly and crashed; gs_reset_blend_state logged
 * the name gs_preprocessor_name.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include "obs.h"
#include "graphics/graphics.h"
#include "graphics/axisang.h"
#include "graphics/matrix4.h"
#include "graphics/quat.h"
#include "graphics/vec3.h"

static char last_log[512];
static int log_count;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)param;
	vsnprintf(last_log, sizeof(last_log), format, args);
	log_count++;
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(NULL, NULL);
	return 0;
}

static void expect_log(const char *name)
{
	char expected[512];
	snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", name);
	assert_string_equal(last_log, expected);
}

#define CALL(name, ...)                       \
	do {                                  \
		last_log[0] = '\0';           \
		int before = log_count;       \
		name(__VA_ARGS__);            \
		assert_int_equal(log_count, before + 1); \
		expect_log(#name);            \
	} while (0)

static void matrix_functions(void **state)
{
	(void)state;
	struct matrix4 m;
	struct quat q;
	struct axisang aa;
	struct vec3 v;
	matrix4_identity(&m);
	quat_identity(&q);
	axisang_set(&aa, 0.0f, 0.0f, 1.0f, 0.5f);
	vec3_set(&v, 1.0f, 2.0f, 3.0f);

	CALL(gs_matrix_push);
	CALL(gs_matrix_pop);
	CALL(gs_matrix_identity);
	CALL(gs_matrix_transpose);
	CALL(gs_matrix_set, &m);
	CALL(gs_matrix_get, &m);
	CALL(gs_matrix_mul, &m);
	CALL(gs_matrix_rotquat, &q);
	CALL(gs_matrix_rotaa, &aa);
	CALL(gs_matrix_translate, &v);
	CALL(gs_matrix_scale, &v);
	CALL(gs_matrix_rotaa4f, 0.0f, 0.0f, 1.0f, 0.5f);
	CALL(gs_matrix_translate3f, 1.0f, 2.0f, 3.0f);
	CALL(gs_matrix_scale3f, 1.0f, 2.0f, 3.0f);
}

static void state_stack_functions(void **state)
{
	(void)state;
	CALL(gs_viewport_push);
	CALL(gs_viewport_pop);
	CALL(gs_blend_state_push);
	CALL(gs_blend_state_pop);
	CALL(gs_reset_blend_state);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(matrix_functions),
		cmocka_unit_test(state_stack_functions),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
