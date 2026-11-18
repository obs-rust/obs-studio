/* Characterize public render-target dispatch against actual libobs without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* The characterization lands with the extraction, before the Rust swap. */
#if !__has_include("graphics/graphics-render-target-setters.c.h")
#error "graphics render-target setters extraction seam is missing"
#endif

enum target_kind { TARGET_2D, TARGET_CUBE };
struct target_call {
	enum target_kind kind;
	gs_device_t *device;
	gs_texture_t *texture;
	gs_zstencil_t *depth;
	int side;
};
static struct graphics_subsystem graphics;
static int device_token, texture_token, depth_token;
static struct target_call calls[64];
static unsigned call_count, log_count;
static char message[160];
static log_handler_t previous_handler;
static void *previous_param;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	assert_int_equal(level, LOG_DEBUG);
	log_count++;
	vsnprintf(message, sizeof(message), format, args);
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static void record(enum target_kind kind, gs_device_t *device, gs_texture_t *texture, gs_zstencil_t *depth, int side)
{
	assert_true(call_count < sizeof(calls) / sizeof(calls[0]));
	calls[call_count++] = (struct target_call){kind, device, texture, depth, side};
}

static void target(gs_device_t *device, gs_texture_t *texture, gs_zstencil_t *depth)
{
	record(TARGET_2D, device, texture, depth, 0);
}

static void cube_target(gs_device_t *device, gs_texture_t *texture, int side, gs_zstencil_t *depth)
{
	record(TARGET_CUBE, device, texture, depth, side);
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	call_count = log_count = 0;
	message[0] = '\0';
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_set_render_target = target;
	graphics.exports.device_set_cube_render_target = cube_target;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_get_log_handler(&previous_handler, &previous_param);
	base_set_log_handler(capture, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(previous_handler, previous_param);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void diagnostic(const char *name, unsigned expected_logs)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", name);
	assert_string_equal(message, expected);
	assert_int_equal(log_count, expected_logs);
	assert_int_equal(call_count, 0);
}

static void no_context(void **state)
{
	(void)state;
	unsigned logs = 0;
	for (unsigned mask = 0; mask < 4; mask++) {
		gs_texture_t *texture = (mask & 1) ? (gs_texture_t *)&texture_token : NULL;
		gs_zstencil_t *depth = (mask & 2) ? (gs_zstencil_t *)&depth_token : NULL;
		gs_set_render_target(texture, depth);
		diagnostic("gs_set_render_target", ++logs);
		gs_set_cube_render_target(texture, INT_MIN, depth);
		diagnostic("gs_set_cube_render_target", ++logs);
	}
}

static void forwarded(unsigned position, enum target_kind kind, gs_texture_t *texture, gs_zstencil_t *depth, int side)
{
	assert_int_equal(call_count, position + 1);
	assert_int_equal(calls[position].kind, kind);
	assert_ptr_equal(calls[position].device, &device_token);
	assert_ptr_equal(calls[position].texture, texture);
	assert_ptr_equal(calls[position].depth, depth);
	assert_int_equal(calls[position].side, side);
	assert_int_equal(log_count, 0);
}

static void nullable_render_targets(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	/* Characterized: neither target nor depth has a pointer guard. */
	for (unsigned mask = 0; mask < 4; mask++) {
		gs_texture_t *texture = (mask & 1) ? (gs_texture_t *)&texture_token : NULL;
		gs_zstencil_t *depth = (mask & 2) ? (gs_zstencil_t *)&depth_token : NULL;
		gs_set_render_target(texture, depth);
		forwarded(mask, TARGET_2D, texture, depth, 0);
	}
	gs_leave_context();
	call_count = 0;
	gs_set_render_target(NULL, NULL);
	diagnostic("gs_set_render_target", 1);
}

static void nullable_cube_targets_and_faces(void **state)
{
	(void)state;
	const int sides[] = {0, 1, 2, 3, 4, 5, -1, INT_MIN, INT_MAX};
	gs_enter_context(&graphics);
	for (unsigned mask = 0; mask < 4; mask++) {
		gs_texture_t *texture = (mask & 1) ? (gs_texture_t *)&texture_token : NULL;
		gs_zstencil_t *depth = (mask & 2) ? (gs_zstencil_t *)&depth_token : NULL;
		/* Characterized, not endorsed: out-of-range faces reach the backend unchanged. */
		for (unsigned i = 0; i < sizeof(sides) / sizeof(sides[0]); i++) {
			unsigned n = call_count;
			gs_set_cube_render_target(texture, sides[i], depth);
			forwarded(n, TARGET_CUBE, texture, depth, sides[i]);
		}
	}
	gs_leave_context();
	call_count = 0;
	gs_set_cube_render_target(NULL, INT_MAX, NULL);
	diagnostic("gs_set_cube_render_target", 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(no_context, setup, teardown),
					   cmocka_unit_test_setup_teardown(nullable_render_targets, setup, teardown),
					   cmocka_unit_test_setup_teardown(nullable_cube_targets_and_faces, setup,
									   teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
