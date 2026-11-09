/* C-only characterization of the private render setup helper. Recording
 * callbacks exercise the actual C body without a graphics device. A future
 * Rust swap must make the helper reachable before reusing this as Tier 2. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <limits.h>
#include <cmocka.h>

#define gs_enable_depth_test render_size_depth
#define gs_set_cull_mode render_size_cull
#define gs_ortho render_size_ortho
#define gs_set_viewport render_size_viewport
#include "libobs/obs-video-render-size.c.h"
#undef gs_enable_depth_test
#undef gs_set_cull_mode
#undef gs_ortho
#undef gs_set_viewport

static unsigned calls;
static uint32_t expected_width, expected_height;

void render_size_depth(bool enable)
{
	assert_int_equal(calls++, 0);
	assert_false(enable);
}

void render_size_cull(enum gs_cull_mode mode)
{
	assert_int_equal(calls++, 1);
	assert_int_equal(mode, GS_NEITHER);
}

void render_size_ortho(float left, float right, float top, float bottom, float znear, float zfar)
{
	assert_int_equal(calls++, 2);
	assert_true(left == 0.0f);
	assert_true(right == (float)expected_width);
	assert_true(top == 0.0f);
	assert_true(bottom == (float)expected_height);
	assert_true(znear == -100.0f);
	assert_true(zfar == 100.0f);
}

void render_size_viewport(int x, int y, int width, int height)
{
	assert_int_equal(calls++, 3);
	assert_int_equal(x, 0);
	assert_int_equal(y, 0);
	assert_int_equal(width, (int)expected_width);
	assert_int_equal(height, (int)expected_height);
}

static void dimensions_and_call_order(void **state)
{
	(void)state;
	const uint32_t sizes[][2] = {
		{0, 0}, {0, 1}, {1, 0}, {1, 1}, {1920, 1080}, {1080, 1920}, {16777215, 16777217}, {INT_MAX, INT_MAX}};
	for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
		expected_width = sizes[i][0];
		expected_height = sizes[i][1];
		calls = 0;
		set_render_size(expected_width, expected_height);
		assert_int_equal(calls, 4);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(dimensions_and_call_order)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
