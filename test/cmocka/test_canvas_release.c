/* Characterize obs_canvas_release through the real libobs ABI before its Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned warnings;
static int warning_level;
static char warning_text[128];

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	warnings++;
	warning_level = level;
	vsnprintf(warning_text, sizeof(warning_text), format, args);
}

static void shutdown_core_guards_before_dereferencing(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	obs_shutdown();
	assert_false(obs_initialized());

	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	warnings = 0;
	base_set_log_handler(capture_log, NULL);
	obs_canvas_release(NULL);
	assert_int_equal(warnings, 0);
	/* Characterized, not endorsed: after shutdown even an invalid non-NULL
	 * handle must only warn, without reading its context or strong counter. */
	obs_canvas_release((obs_canvas_t *)(uintptr_t)1);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(warnings, 1);
	assert_int_equal(warning_level, LOG_WARNING);
	assert_string_equal(warning_text, "Tried to release a canvas when the OBS core is shut down!");
}

static void null_with_live_core_is_silent(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	warnings = 0;
	long allocations = bnum_allocs();
	base_set_log_handler(capture_log, NULL);
	obs_canvas_release(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(warnings, 0);
	assert_int_equal(bnum_allocs(), allocations);
}

struct release_probe {
	obs_canvas_t *canvas;
	obs_weak_canvas_t *control;
	unsigned destroys;
};

static void destroying_canvas(void *data, calldata_t *params)
{
	struct release_probe *probe = data;
	probe->destroys++;
	assert_int_equal(probe->destroys, 1);
	assert_ptr_equal(calldata_ptr(params, "canvas"), probe->canvas);
	assert_ptr_equal(probe->canvas->context.control, probe->control);
	assert_string_equal(obs_canvas_get_name(probe->canvas), "release probe");
	/* The final strong decrement precedes destroy, but the canvas-owned weak
	 * reference is still held throughout the destroy signal. */
	assert_int_equal(os_atomic_load_long(&probe->control->ref.refs), -1);
	assert_int_equal(os_atomic_load_long(&probe->control->ref.weak_refs), 1);
	assert_null(obs_weak_canvas_get_canvas(probe->control));
}

static void nonfinal_and_final_release_order(void **state)
{
	(void)state;
	obs_canvas_t *canvas = obs_canvas_create_private("release probe", NULL, 0);
	assert_non_null(canvas);
	obs_weak_canvas_t *control = obs_canvas_get_weak_canvas(canvas);
	assert_non_null(control);
	assert_ptr_equal(canvas->context.control, control);
	assert_int_equal(os_atomic_load_long(&control->ref.refs), 0);
	assert_int_equal(os_atomic_load_long(&control->ref.weak_refs), 1);
	struct release_probe probe = {.canvas = canvas, .control = control};
	signal_handler_connect(obs_canvas_get_signal_handler(canvas), "destroy", destroying_canvas, &probe);

	assert_ptr_equal(obs_canvas_get_ref(canvas), canvas);
	assert_ptr_equal(obs_canvas_get_ref(canvas), canvas);
	assert_int_equal(os_atomic_load_long(&control->ref.refs), 2);
	for (long remaining = 1; remaining >= 0; remaining--) {
		obs_canvas_release(canvas);
		assert_int_equal(os_atomic_load_long(&control->ref.refs), remaining);
		assert_int_equal(os_atomic_load_long(&control->ref.weak_refs), 1);
		assert_int_equal(probe.destroys, 0);
		assert_ptr_equal(canvas->context.control, control);
		assert_string_equal(obs_canvas_get_name(canvas), "release probe");
	}

	obs_canvas_release(canvas);
	/* Destroy clears/frees the context. Release must use its cached control,
	 * after destroy has returned, rather than reading that freed context. */
	assert_int_equal(probe.destroys, 1);
	assert_int_equal(os_atomic_load_long(&control->ref.refs), -1);
	assert_int_equal(os_atomic_load_long(&control->ref.weak_refs), 0);
	assert_null(obs_weak_canvas_get_canvas(control));
	long allocations = bnum_allocs();
	obs_weak_canvas_release(control);
	assert_int_equal(bnum_allocs(), allocations - 1);
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(shutdown_core_guards_before_dereferencing),
		cmocka_unit_test_setup_teardown(null_with_live_core_is_silent, setup, teardown),
		cmocka_unit_test_setup_teardown(nonfinal_and_final_release_order, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
