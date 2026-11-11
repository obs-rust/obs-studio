/* Characterize the public frame and present exports through a recording backend.
 * No graphics module, GPU allocation, or rendering is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

enum call {
	CALL_BEGIN_FRAME,
	CALL_BEGIN_SCENE,
	CALL_DRAW,
	CALL_END_SCENE,
	CALL_LOAD_SWAPCHAIN,
	CALL_CLEAR,
	CALL_IS_PRESENT_READY,
	CALL_PRESENT,
	CALL_FLUSH,
	CALL_COUNT,
};

#define MAX_EVENTS 64

struct event {
	enum call call;
	gs_device_t *device;
	enum gs_draw_mode draw_mode;
	uint32_t start_vert, num_verts;
	gs_swapchain_t *swapchain;
	uint32_t clear_flags;
	const struct vec4 *color;
	uint32_t depth_bits;
	uint8_t stencil;
};

static struct graphics_subsystem graphics;
static unsigned char devices[2];
static unsigned char swapchains[2];
static gs_device_t *expected_device;
static struct event events[MAX_EVENTS];
static unsigned num_events;
static bool present_ready_answer;
static unsigned logs;
static int last_level;
static char last_log[160];
static log_handler_t saved_handler;
static void *saved_param;

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

static uint32_t float_bits(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

static float bits_float(uint32_t bits)
{
	float value;
	memcpy(&value, &bits, sizeof(value));
	return value;
}

static struct event *record(enum call call, gs_device_t *device)
{
	assert_ptr_equal(device, expected_device);
	assert_true(num_events < MAX_EVENTS);
	struct event *event = &events[num_events++];
	memset(event, 0, sizeof(*event));
	event->call = call;
	event->device = device;
	return event;
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, expected_device);
}

static void backend_begin_frame(gs_device_t *device)
{
	record(CALL_BEGIN_FRAME, device);
}

static void backend_begin_scene(gs_device_t *device)
{
	record(CALL_BEGIN_SCENE, device);
}

static void backend_draw(gs_device_t *device, enum gs_draw_mode draw_mode, uint32_t start_vert, uint32_t num_verts)
{
	struct event *event = record(CALL_DRAW, device);
	event->draw_mode = draw_mode;
	event->start_vert = start_vert;
	event->num_verts = num_verts;
}

static void backend_end_scene(gs_device_t *device)
{
	record(CALL_END_SCENE, device);
}

static void backend_load_swapchain(gs_device_t *device, gs_swapchain_t *swapchain)
{
	record(CALL_LOAD_SWAPCHAIN, device)->swapchain = swapchain;
}

static void backend_clear(gs_device_t *device, uint32_t clear_flags, const struct vec4 *color, float depth,
			  uint8_t stencil)
{
	struct event *event = record(CALL_CLEAR, device);
	event->clear_flags = clear_flags;
	event->color = color;
	event->depth_bits = float_bits(depth);
	event->stencil = stencil;
}

static bool backend_is_present_ready(gs_device_t *device)
{
	record(CALL_IS_PRESENT_READY, device);
	return present_ready_answer;
}

static void backend_present(gs_device_t *device)
{
	record(CALL_PRESENT, device);
}

static void backend_flush(gs_device_t *device)
{
	record(CALL_FLUSH, device);
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	memset(events, 0, sizeof(events));
	num_events = logs = 0;
	present_ready_answer = true;
	expected_device = (gs_device_t *)&devices[0];
	graphics.device = expected_device;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_begin_frame = backend_begin_frame;
	graphics.exports.device_begin_scene = backend_begin_scene;
	graphics.exports.device_draw = backend_draw;
	graphics.exports.device_end_scene = backend_end_scene;
	graphics.exports.device_load_swapchain = backend_load_swapchain;
	graphics.exports.device_clear = backend_clear;
	graphics.exports.device_is_present_ready = backend_is_present_ready;
	graphics.exports.device_present = backend_present;
	graphics.exports.device_flush = backend_flush;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(saved_handler, saved_param);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void diagnostic(const char *name)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", name);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(num_events, 0);
	logs = 0;
}

static const struct vec4 clear_color = {.x = 0.25f, .y = -0.5f, .z = 1.0f, .w = 0.0f};

static void missing_context(void **state)
{
	(void)state;
	/* Check both initially absent and released contexts, with callbacks installed
	 * and the backend's present-ready answer set to true. */
	for (unsigned phase = 0; phase < 2; phase++) {
		if (phase) {
			gs_enter_context(&graphics);
			gs_leave_context();
		}
		assert_null(gs_get_context());
		gs_begin_frame();
		diagnostic("gs_begin_frame");
		gs_begin_scene();
		diagnostic("gs_begin_scene");
		gs_draw(GS_TRIS, 3, 6);
		diagnostic("gs_draw");
		gs_end_scene();
		diagnostic("gs_end_scene");
		gs_load_swapchain((gs_swapchain_t *)&swapchains[0]);
		diagnostic("gs_load_swapchain");
		gs_load_swapchain(NULL);
		diagnostic("gs_load_swapchain");
		gs_clear(GS_CLEAR_COLOR | GS_CLEAR_DEPTH | GS_CLEAR_STENCIL, &clear_color, 1.0f, 0);
		diagnostic("gs_clear");
		gs_clear(0, NULL, 0.0f, 0);
		diagnostic("gs_clear");
		assert_false(gs_is_present_ready());
		diagnostic("gs_is_present_ready");
		gs_present();
		diagnostic("gs_present");
		gs_flush();
		diagnostic("gs_flush");
	}
}

static void expect_only(enum call call)
{
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].call, call);
	assert_ptr_equal(events[0].device, expected_device);
	assert_int_equal(logs, 0);
	num_events = 0;
}

static const enum gs_draw_mode draw_modes[] = {GS_POINTS, GS_LINES, GS_LINESTRIP, GS_TRIS, GS_TRISTRIP};

static void draw_forwarding(void)
{
	const uint32_t ranges[][2] = {{0, 0}, {0, 3}, {7, 1}, {UINT32_MAX, UINT32_MAX}, {1, UINT32_MAX}};
	for (unsigned m = 0; m < sizeof(draw_modes) / sizeof(draw_modes[0]); m++) {
		/* The numeric values are part of the ABI the port must keep. */
		assert_int_equal(draw_modes[m], m);
		for (unsigned r = 0; r < sizeof(ranges) / sizeof(ranges[0]); r++) {
			gs_draw(draw_modes[m], ranges[r][0], ranges[r][1]);
			assert_int_equal(events[0].draw_mode, draw_modes[m]);
			assert_int_equal(events[0].start_vert, ranges[r][0]);
			assert_int_equal(events[0].num_verts, ranges[r][1]);
			expect_only(CALL_DRAW);
		}
	}
	/* No wrapper-side validation of the mode: an undeclared value passes through. */
	gs_draw((enum gs_draw_mode)99, 2, 4);
	assert_int_equal(events[0].draw_mode, 99);
	expect_only(CALL_DRAW);
}

static void clear_forwarding(void)
{
	const uint32_t flags[] = {0,
				  GS_CLEAR_COLOR,
				  GS_CLEAR_DEPTH,
				  GS_CLEAR_STENCIL,
				  GS_CLEAR_COLOR | GS_CLEAR_DEPTH | GS_CLEAR_STENCIL,
				  0xFFFFFFF8u,
				  UINT32_MAX};
	const uint32_t depths[] = {float_bits(0.0f), 0x80000000u, float_bits(1.0f), float_bits(-2.5f),
				   0x7F800000u,      0xFF800000u, 0x7FC00001u,      0x00000001u};
	const uint8_t stencils[] = {0, 1, 0x80, 0xFF};
	const struct vec4 *colors[] = {&clear_color, NULL};
	unsigned n = 0;
	for (unsigned f = 0; f < sizeof(flags) / sizeof(flags[0]); f++) {
		for (unsigned d = 0; d < sizeof(depths) / sizeof(depths[0]); d++) {
			const struct vec4 *color = colors[n % 2];
			uint8_t stencil = stencils[n % (sizeof(stencils) / sizeof(stencils[0]))];
			n++;
			gs_clear(flags[f], color, bits_float(depths[d]), stencil);
			assert_int_equal(events[0].clear_flags, flags[f]);
			/* The colour is passed by pointer, never copied or replaced. */
			assert_ptr_equal(events[0].color, color);
			/* Exact float bits: signed zero, infinities and a NaN payload survive. */
			assert_int_equal(events[0].depth_bits, depths[d]);
			assert_int_equal(events[0].stencil, stencil);
			expect_only(CALL_CLEAR);
		}
	}
}

static void forwarding(void **state)
{
	(void)state;
	_Static_assert(sizeof(enum gs_draw_mode) == sizeof(int), "draw mode enum ABI size");
	for (unsigned device = 0; device < 2; device++) {
		expected_device = (gs_device_t *)&devices[device];
		graphics.device = expected_device;
		gs_enter_context(&graphics);
		assert_ptr_equal(gs_get_context(), &graphics);

		gs_begin_frame();
		expect_only(CALL_BEGIN_FRAME);
		gs_begin_scene();
		expect_only(CALL_BEGIN_SCENE);
		gs_end_scene();
		expect_only(CALL_END_SCENE);
		gs_present();
		expect_only(CALL_PRESENT);
		gs_flush();
		expect_only(CALL_FLUSH);

		for (unsigned s = 0; s < 3; s++) {
			gs_swapchain_t *swapchain = s < 2 ? (gs_swapchain_t *)&swapchains[s] : NULL;
			gs_load_swapchain(swapchain);
			/* NULL is forwarded too: the wrapper does not check the swapchain. */
			assert_ptr_equal(events[0].swapchain, swapchain);
			expect_only(CALL_LOAD_SWAPCHAIN);
		}

		/* The export's answer is returned as is, each time it is asked. */
		present_ready_answer = true;
		assert_true(gs_is_present_ready());
		expect_only(CALL_IS_PRESENT_READY);
		present_ready_answer = false;
		assert_false(gs_is_present_ready());
		expect_only(CALL_IS_PRESENT_READY);
		present_ready_answer = true;
		assert_true(gs_is_present_ready());
		expect_only(CALL_IS_PRESENT_READY);

		draw_forwarding();
		clear_forwarding();

		gs_leave_context();
		assert_null(gs_get_context());
	}
}

static void frame_sequence(void **state)
{
	(void)state;
	/* A typical frame dispatches in call order, one export per call, with no
	 * wrapper-side state: repeated or out-of-order calls are all forwarded. */
	gs_enter_context(&graphics);
	gs_begin_frame();
	gs_load_swapchain((gs_swapchain_t *)&swapchains[1]);
	gs_begin_scene();
	gs_clear(GS_CLEAR_COLOR, &clear_color, 1.0f, 0);
	gs_draw(GS_TRISTRIP, 0, 4);
	gs_end_scene();
	gs_end_scene();
	(void)gs_is_present_ready();
	gs_present();
	gs_present();
	gs_flush();
	gs_begin_scene();
	gs_leave_context();

	const enum call expected[] = {CALL_BEGIN_FRAME, CALL_LOAD_SWAPCHAIN, CALL_BEGIN_SCENE, CALL_CLEAR,
				      CALL_DRAW,        CALL_END_SCENE,      CALL_END_SCENE,   CALL_IS_PRESENT_READY,
				      CALL_PRESENT,     CALL_PRESENT,        CALL_FLUSH,       CALL_BEGIN_SCENE};
	assert_int_equal(num_events, sizeof(expected) / sizeof(expected[0]));
	for (unsigned i = 0; i < num_events; i++) {
		assert_int_equal(events[i].call, expected[i]);
		assert_ptr_equal(events[i].device, expected_device);
	}
	assert_int_equal(logs, 0);
	assert_int_equal(CALL_COUNT, 9);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(missing_context, setup, teardown),
					   cmocka_unit_test_setup_teardown(forwarding, setup, teardown),
					   cmocka_unit_test_setup_teardown(frame_sequence, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
