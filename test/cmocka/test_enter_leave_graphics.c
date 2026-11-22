/* C-only characterization before extracting obs_enter_graphics and
 * obs_leave_graphics (Refs #5459).
 *
 * Both forward to the graphics subsystem only when the core has one:
 * obs_enter_graphics() enters the core's graphics context on the calling
 * thread (gs_enter_context), obs_leave_graphics() leaves the calling thread's
 * current context (gs_leave_context). Before the first obs_reset_video()
 * there is no graphics subsystem and both do nothing at all, not even the
 * "Null 'graphics' parameter" / "not in a graphics context" diagnostics the
 * gs_* calls would log.
 *
 * The core's graphics_t is hidden in libobs (obs is not exported), so the
 * test learns which context is the core's from a main render callback, which
 * runs on the graphics thread inside that context.
 *
 * obs_startup() and obs_reset_video() need a display and OpenGL (run under
 * xvfb-run); without them the cases skip. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <graphics/graphics.h>
#include <util/platform.h>
#include <util/threading.h>

#define CANVAS_SIZE 64
#define TIMEOUT_MS 10000
#define BLOCKED_PROBE_MS 300

#define REQUIRE(cond, why)                                       \
	do {                                                     \
		if (!(cond)) {                                   \
			fprintf(stderr, "skipped: %s\n", (why)); \
			skip();                                  \
		}                                                \
	} while (false)

/* ------------------------------------------------------------------------ */
/* Log capture: counts the gs_enter_context / gs_leave_context diagnostics   */

static log_handler_t prev_handler;
static void *prev_param;
static volatile long enter_diagnostics;
static volatile long leave_diagnostics;

static void capture_log(int lvl, const char *msg, va_list args, void *p)
{
	char text[512];
	va_list copy;

	va_copy(copy, args);
	vsnprintf(text, sizeof(text), msg, copy);
	va_end(copy);

	if (strstr(text, "gs_enter_context"))
		os_atomic_inc_long(&enter_diagnostics);
	if (strstr(text, "gs_leave_context"))
		os_atomic_inc_long(&leave_diagnostics);

	if (prev_handler)
		prev_handler(lvl, msg, args, p);
}

static void start_capture(void)
{
	os_atomic_set_long(&enter_diagnostics, 0);
	os_atomic_set_long(&leave_diagnostics, 0);
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, prev_param);
}

static void stop_capture(void)
{
	base_set_log_handler(prev_handler, prev_param);
}

/* ------------------------------------------------------------------------ */
/* Core lifecycle                                                            */

static struct obs_video_info video_info(void)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = CANVAS_SIZE,
		.base_height = CANVAS_SIZE,
		.output_width = CANVAS_SIZE,
		.output_height = CANVAS_SIZE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return ovi;
}

static int startup_only(void **state)
{
	*state = (void *)(uintptr_t)obs_startup("en-US", NULL, NULL);
	return 0;
}

static int startup_with_video(void **state)
{
	*state = NULL;
	if (!obs_startup("en-US", NULL, NULL))
		return 0;
	struct obs_video_info ovi = video_info();
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}
	*state = (void *)(uintptr_t)1;
	return 0;
}

static int shutdown_core(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

/* The core's graphics context, as the graphics thread sees it while it
 * renders the main texture. */
static graphics_t *volatile render_context;

static void record_render_context(void *param, uint32_t cx, uint32_t cy)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(cx);
	UNUSED_PARAMETER(cy);
	if (!render_context)
		render_context = gs_get_context();
}

static graphics_t *core_graphics(void)
{
	render_context = NULL;
	obs_add_main_render_callback(record_render_context, NULL);
	for (int waited = 0; !render_context && waited < TIMEOUT_MS; waited += 5)
		os_sleep_ms(5);
	obs_remove_main_render_callback(record_render_context, NULL);
	assert_non_null(render_context);
	return render_context;
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

static void no_graphics_before_video_reset(void **state)
{
	REQUIRE(*state, "obs_startup failed");
	assert_null(gs_get_context());

	start_capture();
	for (int i = 0; i < 4; i++) {
		obs_enter_graphics();
		assert_null(gs_get_context());
	}
	for (int i = 0; i < 4; i++) {
		obs_leave_graphics();
		assert_null(gs_get_context());
	}
	stop_capture();

	/* Neither reaches the graphics subsystem: no gs_* diagnostic. */
	assert_int_equal(os_atomic_load_long(&enter_diagnostics), 0);
	assert_int_equal(os_atomic_load_long(&leave_diagnostics), 0);
}

static void enter_makes_core_context_current(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	graphics_t *core = core_graphics();

	assert_null(gs_get_context());

	start_capture();
	obs_enter_graphics();
	assert_ptr_equal(gs_get_context(), core);
	obs_leave_graphics();
	assert_null(gs_get_context());
	stop_capture();

	assert_int_equal(os_atomic_load_long(&enter_diagnostics), 0);
	assert_int_equal(os_atomic_load_long(&leave_diagnostics), 0);

	/* Again, so leaving really released the context for a new entry. */
	obs_enter_graphics();
	assert_ptr_equal(gs_get_context(), core);
	obs_leave_graphics();
	assert_null(gs_get_context());
}

static void enter_is_counted(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	graphics_t *core = core_graphics();

	obs_enter_graphics();
	obs_enter_graphics();
	obs_enter_graphics();
	assert_ptr_equal(gs_get_context(), core);
	obs_leave_graphics();
	assert_ptr_equal(gs_get_context(), core);
	obs_leave_graphics();
	assert_ptr_equal(gs_get_context(), core);
	obs_leave_graphics();
	assert_null(gs_get_context());

	/* With a graphics subsystem, leaving while outside is forwarded: the
	 * gs layer reports it and nothing changes. */
	start_capture();
	obs_leave_graphics();
	stop_capture();
	assert_null(gs_get_context());
	assert_int_equal(os_atomic_load_long(&leave_diagnostics), 1);
	assert_int_equal(os_atomic_load_long(&enter_diagnostics), 0);

	/* And it did not unbalance the next entry. */
	obs_enter_graphics();
	assert_ptr_equal(gs_get_context(), core);
	obs_leave_graphics();
	assert_null(gs_get_context());
}

struct waiter {
	volatile long entered;
	volatile long done;
	graphics_t *seen;
};

static void *enter_from_thread(void *param)
{
	struct waiter *w = param;

	obs_enter_graphics();
	w->seen = gs_get_context();
	os_atomic_set_long(&w->entered, 1);
	obs_leave_graphics();
	if (gs_get_context())
		w->seen = NULL;
	os_atomic_set_long(&w->done, 1);
	return NULL;
}

static void other_thread_waits_for_leave(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	graphics_t *core = core_graphics();
	struct waiter w = {0};
	pthread_t thread;

	obs_enter_graphics();
	assert_ptr_equal(gs_get_context(), core);
	assert_int_equal(pthread_create(&thread, NULL, enter_from_thread, &w), 0);

	/* While this thread is inside, the other one cannot enter. */
	os_sleep_ms(BLOCKED_PROBE_MS);
	assert_int_equal(os_atomic_load_long(&w.entered), 0);

	/* A nested entry here does not let it in either. */
	obs_enter_graphics();
	obs_leave_graphics();
	os_sleep_ms(BLOCKED_PROBE_MS / 3);
	assert_int_equal(os_atomic_load_long(&w.entered), 0);

	obs_leave_graphics();
	assert_null(gs_get_context());

	for (int waited = 0; !os_atomic_load_long(&w.done) && waited < TIMEOUT_MS; waited += 5)
		os_sleep_ms(5);
	assert_int_equal(os_atomic_load_long(&w.done), 1);
	pthread_join(thread, NULL);

	/* It got the same core context, and left it again. */
	assert_ptr_equal(w.seen, core);
	assert_null(gs_get_context());
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_graphics_before_video_reset, startup_only, shutdown_core),
		cmocka_unit_test_setup_teardown(enter_makes_core_context_current, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(enter_is_counted, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(other_thread_waits_for_leave, startup_with_video, shutdown_core),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
