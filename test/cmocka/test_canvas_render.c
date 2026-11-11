/* Actual-library characterization of obs_canvas_render before its Rust swap.
 * NULL canvas is outside the C domain: &canvas->view forms a member address
 * through NULL (characterized, not endorsed). Unlike obs_view_render(NULL),
 * this wrapper has no NULL guard. Linux isolates that fault in a child;
 * other hosts restrict runtime assertions to the valid non-NULL domain.
 * Rendering uses a started core, but no video reset, graphics module, GPU
 * or elapsed-time checks. The NULL probe runs first, before core threads. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include <errno.h>
#include "obs-internal.h"
#include "graphics/graphics-internal.h"

_Static_assert(offsetof(struct obs_canvas, view) > 0, "NULL canvas does not designate a NULL view");

static struct graphics_subsystem graphics;
static struct obs_canvas canvas[2];
static struct obs_source sources[2];
static struct obs_weak_source controls[2];
static unsigned char device_token;
static unsigned enters, leaves, renders, spaces, markers, marker_ends;
static unsigned order[MAX_CHANNELS];
static struct obs_canvas *expected_canvas;
static bool change_later_channel;
static log_handler_t saved_log;
static void *saved_log_param;
static unsigned logs;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void record_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	logs++;
}

static void enter(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	enters++;
}

static void leave(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	leaves++;
}

static enum gs_color_space color_space(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	spaces++;
	return GS_CS_SRGB;
}

static void marker(gs_device_t *device, const char *name, const float color[4])
{
	assert_ptr_equal(device, &device_token);
	assert_non_null(name);
	assert_non_null(color);
	markers++;
}

static void marker_end(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	marker_ends++;
}

static void render(void *data, gs_effect_t *effect)
{
	unsigned id = *(unsigned *)data;
	assert_null(effect);
	assert_true(renders < MAX_CHANNELS);
	order[renders++] = id;
	/* The original view renderer holds THIS canvas's view mutex and takes
	 * exactly one temporary source reference while invoking the callback. */
	assert_int_equal(controls[id].ref.refs, 11);
	assert_ptr_equal(expected_canvas->view.channels[0], &sources[0]);
	assert_int_equal(pthread_mutex_trylock(&expected_canvas->view.channels_mutex), EBUSY);
	if (change_later_channel && id == 0)
		expected_canvas->view.channels[MAX_CHANNELS - 1] = &sources[1];
	/* Observable callback effects must survive the wrapper. */
	expected_canvas->flags++;
	if (gs_get_context())
		gs_set_linear_srgb(false);
}

static unsigned ids[] = {0, 1};

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	assert_true(obs_startup("en-US", NULL, NULL));
	memset(canvas, 0, sizeof(canvas));
	memset(sources, 0, sizeof(sources));
	memset(controls, 0, sizeof(controls));
	memset(&graphics, 0, sizeof(graphics));
	for (size_t i = 0; i < 2; i++) {
		assert_int_equal(pthread_mutex_init(&canvas[i].view.channels_mutex, NULL), 0);
		canvas[i].flags = UINT32_MAX;
		canvas[i].view.type = AUX_VIEW;
		sources[i].context.type = OBS_OBJ_TYPE_SOURCE;
		sources[i].context.name = "canvas render probe";
		sources[i].context.data = &ids[i];
		sources[i].context.control = (struct obs_weak_object *)&controls[i];
		controls[i].source = &sources[i];
		controls[i].ref.refs = 10;
		sources[i].enabled = true;
		sources[i].info.type = OBS_SOURCE_TYPE_INPUT;
		sources[i].info.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW | OBS_SOURCE_SRGB;
		sources[i].info.video_render = render;
	}
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = enter;
	graphics.exports.device_leave_context = leave;
	graphics.exports.device_get_color_space = color_space;
	graphics.exports.device_debug_marker_begin = marker;
	graphics.exports.device_debug_marker_end = marker_end;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	enters = leaves = renders = spaces = markers = marker_ends = logs = 0;
	change_later_channel = false;
	base_get_log_handler(&saved_log, &saved_log_param);
	base_set_log_handler(record_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(saved_log, saved_log_param);
	for (size_t i = 0; i < 2; i++) {
		assert_int_equal(pthread_mutex_trylock(&canvas[i].view.channels_mutex), 0);
		assert_int_equal(pthread_mutex_unlock(&canvas[i].view.channels_mutex), 0);
		assert_int_equal(pthread_mutex_destroy(&canvas[i].view.channels_mutex), 0);
	}
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	obs_shutdown();
	return 0;
}

#ifdef CANVAS_RENDER_NULL_PROBE
extern void canvas_render_null_probe(void);

static void null_canvas(void **state)
{
	(void)state;
	canvas_render_null_probe();
}
#endif

static void empty_view(void **state)
{
	(void)state;
	/* All other canvas fields are inert, even without a graphics device. */
	obs_canvas_render(&canvas[1]);
	assert_null(gs_get_context());
	assert_int_equal(renders, 0);
	assert_int_equal(logs, 0);
	assert_int_equal(enters, 0);
	assert_int_equal(leaves, 0);
	assert_int_equal(canvas[1].flags, UINT32_MAX);
	assert_int_equal(canvas[1].view.type, AUX_VIEW);
	for (size_t i = 0; i < MAX_CHANNELS; i++)
		assert_null(canvas[1].view.channels[i]);
	gs_enter_context(&graphics);
	graphics.linear_srgb = true;
	obs_canvas_render(&canvas[1]);
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_true(graphics.linear_srgb);
	assert_int_equal(graphics.ref, 1);
	assert_int_equal(enters, 1);
	assert_int_equal(leaves, 0);
	assert_int_equal(renders, 0);
	assert_int_equal(spaces, 0);
	assert_int_equal(markers, 0);
	assert_int_equal(marker_ends, 0);
	assert_int_equal(logs, 0);
}

static void delegation(void **state)
{
	(void)state;
	for (unsigned with_graphics = 0; with_graphics < 2; with_graphics++) {
		if (with_graphics)
			gs_enter_context(&graphics);
		/* Compare the wrapper to the real view export, with identical inputs.
		 * The untouched second canvas catches selecting any other view. */
		unsigned direct_logs = 0, direct_markers = 0;
		for (unsigned direct = 0; direct < 2; direct++) {
			expected_canvas = &canvas[1];
			memset(expected_canvas->view.channels, 0, sizeof(expected_canvas->view.channels));
			expected_canvas->view.channels[0] = &sources[0];
			expected_canvas->view.channels[2] = &sources[1];
			expected_canvas->view.channels[3] = &sources[0];
			expected_canvas->flags = 16;
			change_later_channel = true;
			renders = spaces = markers = marker_ends = logs = 0;
			graphics.linear_srgb = true;
			if (direct)
				obs_canvas_render(expected_canvas);
			else
				obs_view_render(&expected_canvas->view);
			assert_int_equal(renders, 4);
			assert_int_equal(order[0], 0);
			assert_int_equal(order[1], 1);
			assert_int_equal(order[2], 0);
			assert_int_equal(order[3], 1);
			assert_int_equal(expected_canvas->flags, 20);
			assert_int_equal(controls[0].ref.refs, 10);
			assert_int_equal(controls[1].ref.refs, 10);
			assert_int_equal(expected_canvas->view.type, AUX_VIEW);
			assert_ptr_equal(expected_canvas->view.channels[MAX_CHANNELS - 1], &sources[1]);
			for (size_t i = 0; i < MAX_CHANNELS; i++)
				assert_null(canvas[0].view.channels[i]);
			assert_ptr_equal(gs_get_context(), with_graphics ? &graphics : NULL);
			assert_int_equal(enters, with_graphics);
			assert_int_equal(leaves, 0);
			assert_int_equal(graphics.ref, with_graphics);
			if (with_graphics) {
				assert_int_equal(spaces, 4);
				assert_false(graphics.linear_srgb);
				assert_int_equal(graphics.cur_matrix, 0);
				assert_int_equal(graphics.matrix_stack.num, 0);
				assert_int_equal(graphics.viewport_stack.num, 0);
				assert_null(graphics.cur_effect);
			} else {
				assert_int_equal(spaces, 0);
			}
			if (!direct) {
				direct_logs = logs;
				direct_markers = markers;
			} else {
				assert_int_equal(logs, direct_logs);
				assert_int_equal(markers, direct_markers);
			}
			assert_int_equal(markers, marker_ends);
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
#ifdef CANVAS_RENDER_NULL_PROBE
		cmocka_unit_test(null_canvas),
#endif
		cmocka_unit_test_setup_teardown(empty_view, setup, teardown),
		cmocka_unit_test_setup_teardown(delegation, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
