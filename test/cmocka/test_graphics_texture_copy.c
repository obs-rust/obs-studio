/* Characterize public texture-copy dispatch against actual libobs without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* The characterization lands with the extraction, before the Rust swap. */
#if !__has_include("graphics/graphics-texture-copy.c.h")
#error "graphics texture-copy extraction seam is missing"
#endif

enum copy_kind { COPY_WHOLE, COPY_REGION };
struct copy_call {
	enum copy_kind kind;
	gs_device_t *device;
	gs_texture_t *dst, *src;
	uint32_t dst_x, dst_y, src_x, src_y, src_w, src_h;
};
static struct graphics_subsystem graphics;
static int device_token, dst_token, src_token;
static struct copy_call calls[64];
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

static void copy_region(gs_device_t *device, gs_texture_t *dst, uint32_t dst_x, uint32_t dst_y, gs_texture_t *src,
			uint32_t src_x, uint32_t src_y, uint32_t src_w, uint32_t src_h)
{
	assert_true(call_count < sizeof(calls) / sizeof(calls[0]));
	calls[call_count++] =
		(struct copy_call){COPY_REGION, device, dst, src, dst_x, dst_y, src_x, src_y, src_w, src_h};
}

static void copy_whole(gs_device_t *device, gs_texture_t *dst, gs_texture_t *src)
{
	copy_region(device, dst, 0, 0, src, 0, 0, 0, 0);
	calls[call_count - 1].kind = COPY_WHOLE;
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
	graphics.exports.device_copy_texture = copy_whole;
	graphics.exports.device_copy_texture_region = copy_region;
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

static void diagnostic(const char *expected, unsigned expected_logs)
{
	assert_string_equal(message, expected);
	assert_int_equal(log_count, expected_logs);
	assert_int_equal(call_count, 0);
}

static void context_before_pointers(void **state)
{
	(void)state;
	unsigned logs = 0;
	for (unsigned mask = 0; mask < 4; mask++) {
		gs_texture_t *dst = (mask & 1) ? (gs_texture_t *)&dst_token : NULL;
		gs_texture_t *src = (mask & 2) ? (gs_texture_t *)&src_token : NULL;
		gs_copy_texture(dst, src);
		diagnostic("gs_copy_texture: called while not in a graphics context", ++logs);
		gs_copy_texture_region(dst, 1, 2, src, 3, 4, 5, 6);
		diagnostic("gs_copy_texture_region: called while not in a graphics context", ++logs);
	}
}

static void pointer_guards(void **state)
{
	(void)state;
	gs_texture_t *dst = (gs_texture_t *)&dst_token;
	gs_texture_t *src = (gs_texture_t *)&src_token;
	gs_enter_context(&graphics);
	gs_copy_texture(NULL, NULL);
	diagnostic("gs_copy_texture: Null 'dst' parameter", 1);
	gs_copy_texture(NULL, src);
	diagnostic("gs_copy_texture: Null 'dst' parameter", 2);
	gs_copy_texture(dst, NULL);
	diagnostic("gs_copy_texture: Null 'src' parameter", 3);
	gs_copy_texture_region(NULL, 1, 2, NULL, 3, 4, 5, 6);
	diagnostic("gs_copy_texture_region: Null 'dst' parameter", 4);
	gs_copy_texture_region(NULL, 1, 2, src, 3, 4, 5, 6);
	diagnostic("gs_copy_texture_region: Null 'dst' parameter", 5);
}

static void forwarded(unsigned position, enum copy_kind kind, gs_texture_t *dst, gs_texture_t *src,
		      const uint32_t *values)
{
	assert_int_equal(call_count, position + 1);
	assert_int_equal(calls[position].kind, kind);
	assert_ptr_equal(calls[position].device, &device_token);
	assert_ptr_equal(calls[position].dst, dst);
	assert_ptr_equal(calls[position].src, src);
	assert_int_equal(calls[position].dst_x, values[0]);
	assert_int_equal(calls[position].dst_y, values[1]);
	assert_int_equal(calls[position].src_x, values[2]);
	assert_int_equal(calls[position].src_y, values[3]);
	assert_int_equal(calls[position].src_w, values[4]);
	assert_int_equal(calls[position].src_h, values[5]);
	assert_int_equal(log_count, 0);
}

static void whole_forwarding(void **state)
{
	(void)state;
	gs_texture_t *dst = (gs_texture_t *)&dst_token;
	gs_texture_t *src = (gs_texture_t *)&src_token;
	const uint32_t zeros[6] = {0};
	gs_enter_context(&graphics);
	gs_copy_texture(dst, src);
	forwarded(0, COPY_WHOLE, dst, src, zeros);
	/* Characterized: an aliased source and destination still dispatch once. */
	gs_copy_texture(dst, dst);
	forwarded(1, COPY_WHOLE, dst, dst, zeros);
	gs_leave_context();
	call_count = 0;
	gs_copy_texture(dst, src);
	diagnostic("gs_copy_texture: called while not in a graphics context", 1);
}

static void region_forwarding(void **state)
{
	(void)state;
	gs_texture_t *dst = (gs_texture_t *)&dst_token;
	const uint32_t values[][6] = {{11, 23, 37, 41, 53, 67},    {0, 0, 0, 0, 0, 0},
				      {UINT32_MAX, 1, 2, 3, 4, 5}, {1, UINT32_MAX, 2, 3, 4, 5},
				      {1, 2, UINT32_MAX, 3, 4, 5}, {1, 2, 3, UINT32_MAX, 4, 5},
				      {1, 2, 3, 4, UINT32_MAX, 5}, {1, 2, 3, 4, 5, UINT32_MAX}};
	gs_texture_t *sources[] = {NULL, (gs_texture_t *)&src_token, dst};
	gs_enter_context(&graphics);
	/* Characterized, not endorsed: NULL source and unchecked extents reach the backend. */
	for (unsigned s = 0; s < sizeof(sources) / sizeof(sources[0]); s++) {
		for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
			const uint32_t *v = values[i];
			unsigned n = call_count;
			gs_copy_texture_region(dst, v[0], v[1], sources[s], v[2], v[3], v[4], v[5]);
			forwarded(n, COPY_REGION, dst, sources[s], v);
		}
	}
	gs_leave_context();
	call_count = 0;
	gs_copy_texture_region(dst, 1, 2, NULL, 3, 4, 5, 6);
	diagnostic("gs_copy_texture_region: called while not in a graphics context", 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(context_before_pointers, setup, teardown),
					   cmocka_unit_test_setup_teardown(pointer_guards, setup, teardown),
					   cmocka_unit_test_setup_teardown(whole_forwarding, setup, teardown),
					   cmocka_unit_test_setup_teardown(region_forwarding, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
