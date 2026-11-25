/* Actual-library characterization of the main-render registration exports
 * (Refs #5705, #5172). Unlike test_main_render_callbacks, this never includes
 * or renames the implementation. No video is started: NULL callback tokens
 * are characterized as stored data, not endorsed as callable functions. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static struct obs_core *core;
static bool seeded_tail;
static unsigned draw_calls;
static int token_a, token_b, token_tail;

static void draw_one(void *param, uint32_t cx, uint32_t cy)
{
	(void)param;
	(void)cx;
	(void)cy;
	draw_calls++;
}

static void draw_two(void *param, uint32_t cx, uint32_t cy)
{
	(void)param;
	(void)cx;
	(void)cy;
	draw_calls += 2;
}

static void expect_list(const struct draw_callback *expected, size_t count)
{
	struct draw_callback actual[16];
	assert_int_equal(pthread_mutex_lock(&core->data.draw_callbacks_mutex), 0);
	size_t num = core->data.draw_callbacks.num;
	if (num <= 16 && num)
		memcpy(actual, core->data.draw_callbacks.array, num * sizeof(*actual));
	size_t rendered_num = core->data.rendered_callbacks.num;
	assert_int_equal(pthread_mutex_unlock(&core->data.draw_callbacks_mutex), 0);
	assert_true(num <= 16);
	assert_int_equal(num, count + (seeded_tail ? 1 : 0));
	for (size_t i = 0; i < count; i++) {
		assert_true(actual[i].draw == expected[i].draw);
		assert_ptr_equal(actual[i].param, expected[i].param);
	}
	if (seeded_tail) {
		assert_true(actual[count].draw == draw_one);
		assert_ptr_equal(actual[count].param, &token_tail);
	}
	assert_int_equal(draw_calls, 0);
	assert_int_equal(rendered_num, 0);
}

static int setup_with(void **state, bool seeded)
{
	(void)state;
	/* obs is hidden in the shared library. Canvas insertion stores the real
	 * core's canvases_mutex address in context.mutex. Recover its containing
	 * core using the real private header, without interposing any symbol or
	 * compiling a second implementation. This pointer is observation-only. */
	obs_canvas_t *canvas = obs_get_main_canvas();
	assert_non_null(canvas);
	assert_non_null(canvas->context.mutex);
	core = (struct obs_core *)((unsigned char *)canvas->context.mutex - offsetof(struct obs_core, data) -
				   offsetof(struct obs_core_data, canvases_mutex));
	assert_ptr_equal(core->data.main_canvas, canvas);
	assert_ptr_equal(core->signals, obs_get_signal_handler());
	seeded_tail = false;
	draw_calls = 0;
	expect_list(NULL, 0);
	if (seeded) {
		obs_add_main_render_callback(draw_one, &token_tail);
		seeded_tail = true;
		expect_list(NULL, 0);
	}
	return 0;
}

static int setup_empty(void **state)
{
	return setup_with(state, false);
}

static int setup_seeded(void **state)
{
	return setup_with(state, true);
}

static int teardown(void **state)
{
	(void)state;
	/* Use the exported removal path, including after a failed assertion.
	 * obs_shutdown owns the array allocation and frees it normally. */
	size_t remaining = core->data.draw_callbacks.num;
	while (remaining-- && core->data.draw_callbacks.num) {
		struct draw_callback entry = core->data.draw_callbacks.array[0];
		obs_remove_main_render_callback(entry.draw, entry.param);
	}
	seeded_tail = false;
	expect_list(NULL, 0);
	core = NULL;
	return 0;
}

static void prepends_without_calling(void **state)
{
	(void)state;
	obs_add_main_render_callback(draw_one, &token_a);
	expect_list((struct draw_callback[]){{draw_one, &token_a}}, 1);
	obs_add_main_render_callback(draw_two, &token_a);
	obs_add_main_render_callback(draw_one, &token_b);
	expect_list((struct draw_callback[]){{draw_one, &token_b}, {draw_two, &token_a}, {draw_one, &token_a}}, 3);
}

static void duplicates_and_first_match_removal(void **state)
{
	(void)state;
	/* Stored order C, A, B, A distinguishes first from last/all removal. */
	obs_add_main_render_callback(draw_one, &token_a);
	obs_add_main_render_callback(draw_two, &token_a);
	obs_add_main_render_callback(draw_one, &token_a);
	obs_add_main_render_callback(draw_two, &token_b);
	expect_list((struct draw_callback[]){{draw_two, &token_b},
					     {draw_one, &token_a},
					     {draw_two, &token_a},
					     {draw_one, &token_a}},
		    4);
	obs_remove_main_render_callback(draw_one, &token_a);
	expect_list((struct draw_callback[]){{draw_two, &token_b}, {draw_two, &token_a}, {draw_one, &token_a}}, 3);
	/* Each component must match; sharing just one component is insufficient. */
	obs_remove_main_render_callback(draw_one, &token_b);
	obs_remove_main_render_callback(draw_two, NULL);
	expect_list((struct draw_callback[]){{draw_two, &token_b}, {draw_two, &token_a}, {draw_one, &token_a}}, 3);
	obs_remove_main_render_callback(draw_one, &token_a);
	obs_remove_main_render_callback(draw_one, &token_a);
	expect_list((struct draw_callback[]){{draw_two, &token_b}, {draw_two, &token_a}}, 2);
}

static void absent_removal_is_a_noop(void **state)
{
	(void)state;
	for (int i = 0; i < 3; i++) {
		obs_remove_main_render_callback(draw_one, &token_a);
		obs_remove_main_render_callback(NULL, NULL);
		expect_list(NULL, 0);
	}
}

static void null_tokens_are_data(void **state)
{
	(void)state;
	obs_add_main_render_callback(NULL, NULL);
	obs_add_main_render_callback(draw_one, NULL);
	obs_add_main_render_callback(NULL, &token_a);
	expect_list((struct draw_callback[]){{NULL, &token_a}, {draw_one, NULL}, {NULL, NULL}}, 3);
	obs_remove_main_render_callback(NULL, NULL);
	expect_list((struct draw_callback[]){{NULL, &token_a}, {draw_one, NULL}}, 2);
	obs_remove_main_render_callback(NULL, &token_a);
	obs_remove_main_render_callback(draw_one, NULL);
	expect_list(NULL, 0);
}

static void removal_can_reach_the_tail(void **state)
{
	(void)state;
	obs_add_main_render_callback(draw_two, &token_a);
	obs_remove_main_render_callback(draw_one, &token_tail);
	seeded_tail = false;
	expect_list((struct draw_callback[]){{draw_two, &token_a}}, 1);
	obs_remove_main_render_callback(draw_two, &token_a);
	expect_list(NULL, 0);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(prepends_without_calling, setup_empty, teardown),
		cmocka_unit_test_setup_teardown(prepends_without_calling, setup_seeded, teardown),
		cmocka_unit_test_setup_teardown(duplicates_and_first_match_removal, setup_empty, teardown),
		cmocka_unit_test_setup_teardown(duplicates_and_first_match_removal, setup_seeded, teardown),
		cmocka_unit_test_setup_teardown(absent_removal_is_a_noop, setup_empty, teardown),
		cmocka_unit_test_setup_teardown(absent_removal_is_a_noop, setup_seeded, teardown),
		cmocka_unit_test_setup_teardown(null_tokens_are_data, setup_empty, teardown),
		cmocka_unit_test_setup_teardown(null_tokens_are_data, setup_seeded, teardown),
		cmocka_unit_test_setup_teardown(removal_can_reach_the_tail, setup_seeded, teardown),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
