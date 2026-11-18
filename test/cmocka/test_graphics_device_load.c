/* Characterize public device-load dispatch against actual libobs without a GPU. */
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
#if !__has_include("graphics/graphics-device-load.c.h")
#error "graphics device-load extraction seam is missing"
#endif

enum load_kind { LOAD_VERTEX, LOAD_INDEX, LOAD_TEXTURE, LOAD_SAMPLER, LOAD_DEFAULT };
struct load_call {
	enum load_kind kind;
	gs_device_t *device;
	const void *object;
	int unit;
	bool b_3d;
};
static struct graphics_subsystem graphics;
static int device_token, vertex_token, index_token, texture_token, sampler_token;
static struct load_call calls[64];
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

static void record(enum load_kind kind, gs_device_t *device, const void *object, int unit, bool b_3d)
{
	assert_true(call_count < sizeof(calls) / sizeof(calls[0]));
	calls[call_count++] = (struct load_call){kind, device, object, unit, b_3d};
}

static void vertex(gs_device_t *device, gs_vertbuffer_t *buffer)
{
	record(LOAD_VERTEX, device, buffer, 0, false);
}

static void index_buffer(gs_device_t *device, gs_indexbuffer_t *buffer)
{
	record(LOAD_INDEX, device, buffer, 0, false);
}

static void texture(gs_device_t *device, gs_texture_t *tex, int unit)
{
	record(LOAD_TEXTURE, device, tex, unit, false);
}

static void sampler(gs_device_t *device, gs_samplerstate_t *state, int unit)
{
	record(LOAD_SAMPLER, device, state, unit, false);
}

static void default_sampler(gs_device_t *device, bool b_3d, int unit)
{
	record(LOAD_DEFAULT, device, NULL, unit, b_3d);
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
	graphics.exports.device_load_vertexbuffer = vertex;
	graphics.exports.device_load_indexbuffer = index_buffer;
	graphics.exports.device_load_texture = texture;
	graphics.exports.device_load_samplerstate = sampler;
	graphics.exports.device_load_default_samplerstate = default_sampler;
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
	for (unsigned nonnull = 0; nonnull < 2; nonnull++) {
		gs_load_vertexbuffer(nonnull ? (gs_vertbuffer_t *)&vertex_token : NULL);
		diagnostic("gs_load_vertexbuffer", nonnull * 5 + 1);
		gs_load_indexbuffer(nonnull ? (gs_indexbuffer_t *)&index_token : NULL);
		diagnostic("gs_load_indexbuffer", nonnull * 5 + 2);
		gs_load_texture(nonnull ? (gs_texture_t *)&texture_token : NULL, -3);
		diagnostic("gs_load_texture", nonnull * 5 + 3);
		gs_load_samplerstate(nonnull ? (gs_samplerstate_t *)&sampler_token : NULL, 7);
		diagnostic("gs_load_samplerstate", nonnull * 5 + 4);
		gs_load_default_samplerstate(nonnull != 0, INT_MAX);
		diagnostic("gs_load_default_samplerstate", nonnull * 5 + 5);
	}
}

static void forwarded(unsigned position, enum load_kind kind, const void *object, int unit, bool b_3d)
{
	assert_int_equal(call_count, position + 1);
	assert_int_equal(calls[position].kind, kind);
	assert_ptr_equal(calls[position].device, &device_token);
	assert_ptr_equal(calls[position].object, object);
	assert_int_equal(calls[position].unit, unit);
	assert_int_equal(calls[position].b_3d, b_3d);
	assert_int_equal(log_count, 0);
}

static void buffers_and_unbinds(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	gs_load_vertexbuffer((gs_vertbuffer_t *)&vertex_token);
	forwarded(0, LOAD_VERTEX, &vertex_token, 0, false);
	gs_load_indexbuffer((gs_indexbuffer_t *)&index_token);
	forwarded(1, LOAD_INDEX, &index_token, 0, false);
	/* Characterized: NULL is forwarded as an unbind, with no pointer guard. */
	gs_load_vertexbuffer(NULL);
	forwarded(2, LOAD_VERTEX, NULL, 0, false);
	gs_load_indexbuffer(NULL);
	forwarded(3, LOAD_INDEX, NULL, 0, false);
	gs_leave_context();
	call_count = 0;
	gs_load_vertexbuffer(NULL);
	diagnostic("gs_load_vertexbuffer", 1);
}

static void texture_sampler_parameters(void **state)
{
	(void)state;
	const int units[] = {0, 3, -1, INT_MIN, INT_MAX};
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(units) / sizeof(units[0]); i++) {
		unsigned n = call_count;
		gs_load_texture((gs_texture_t *)&texture_token, units[i]);
		forwarded(n++, LOAD_TEXTURE, &texture_token, units[i], false);
		gs_load_samplerstate((gs_samplerstate_t *)&sampler_token, units[i]);
		forwarded(n++, LOAD_SAMPLER, &sampler_token, units[i], false);
		gs_load_texture(NULL, units[i]);
		forwarded(n++, LOAD_TEXTURE, NULL, units[i], false);
		gs_load_samplerstate(NULL, units[i]);
		forwarded(n++, LOAD_SAMPLER, NULL, units[i], false);
		gs_load_default_samplerstate(false, units[i]);
		forwarded(n++, LOAD_DEFAULT, NULL, units[i], false);
		gs_load_default_samplerstate(true, units[i]);
		forwarded(n, LOAD_DEFAULT, NULL, units[i], true);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(no_context, setup, teardown),
					   cmocka_unit_test_setup_teardown(buffers_and_unbinds, setup, teardown),
					   cmocka_unit_test_setup_teardown(texture_sampler_parameters, setup,
									   teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
