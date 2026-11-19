/* Characterize the public GPU metadata queries against actual libobs without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"
#include "util/bmem.h"

/* The characterization lands with the extraction, before the Rust swap. */
#if !__has_include("graphics/graphics-gpu-metadata.c.h")
#error "graphics GPU-metadata extraction seam is missing"
#endif

static struct graphics_subsystem graphics;
static int device_token;
static const char *driver_version, *renderer;
static uint64_t dmem, smem;
static unsigned driver_calls, renderer_calls, dmem_calls, smem_calls, log_count;
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

static const char *query_driver_version(void)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	driver_calls++;
	return driver_version;
}

static const char *query_renderer(void)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	renderer_calls++;
	return renderer;
}

static uint64_t query_dmem(void)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	dmem_calls++;
	return dmem;
}

static uint64_t query_smem(void)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	smem_calls++;
	return smem;
}

static void install_callbacks(void)
{
	graphics.exports.gpu_get_driver_version = query_driver_version;
	graphics.exports.gpu_get_renderer = query_renderer;
	graphics.exports.gpu_get_dmem = query_dmem;
	graphics.exports.gpu_get_smem = query_smem;
}

static void clear_callbacks(void)
{
	graphics.exports.gpu_get_driver_version = NULL;
	graphics.exports.gpu_get_renderer = NULL;
	graphics.exports.gpu_get_dmem = NULL;
	graphics.exports.gpu_get_smem = NULL;
}

static void assert_calls(unsigned driver, unsigned render, unsigned d, unsigned s)
{
	assert_int_equal(driver_calls, driver);
	assert_int_equal(renderer_calls, render);
	assert_int_equal(dmem_calls, d);
	assert_int_equal(smem_calls, s);
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	driver_calls = renderer_calls = dmem_calls = smem_calls = log_count = 0;
	message[0] = '\0';
	driver_version = "driver 1.2.3";
	renderer = "renderer X";
	dmem = 1024;
	smem = 2048;
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	install_callbacks();
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

static void no_context(void **state)
{
	(void)state;
	/* With callbacks present or absent, validate context before dispatch. */
	for (unsigned i = 0; i < 2; i++) {
		assert_null(gs_get_driver_version());
		assert_string_equal(message, "gs_get_driver_version: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 1);
		assert_null(gs_get_renderer());
		assert_string_equal(message, "gs_get_renderer: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 2);
		assert_true(gs_get_gpu_dmem() == 0);
		assert_string_equal(message, "gs_get_gpu_dmem: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 3);
		assert_true(gs_get_gpu_smem() == 0);
		assert_string_equal(message, "gs_get_gpu_smem: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 4);
		assert_calls(0, 0, 0, 0);
		clear_callbacks();
	}
}

static void absent_callbacks(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	clear_callbacks();
	/* Each optional backend entry falls back silently to its default. */
	assert_null(gs_get_driver_version());
	assert_null(gs_get_renderer());
	assert_true(gs_get_gpu_dmem() == 0);
	assert_true(gs_get_gpu_smem() == 0);
	assert_calls(0, 0, 0, 0);
	assert_int_equal(log_count, 0);
	assert_string_equal(message, "");
}

static void each_slot_dispatches_alone(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	/* Distinct callbacks prove each entry reads only its own slot, once. */
	assert_ptr_equal(gs_get_driver_version(), driver_version);
	assert_calls(1, 0, 0, 0);
	assert_ptr_equal(gs_get_renderer(), renderer);
	assert_calls(1, 1, 0, 0);
	assert_true(gs_get_gpu_dmem() == 1024);
	assert_calls(1, 1, 1, 0);
	assert_true(gs_get_gpu_smem() == 2048);
	assert_calls(1, 1, 1, 1);

	/* Removing one slot leaves the others dispatching. */
	graphics.exports.gpu_get_renderer = NULL;
	graphics.exports.gpu_get_dmem = NULL;
	assert_ptr_equal(gs_get_driver_version(), driver_version);
	assert_null(gs_get_renderer());
	assert_true(gs_get_gpu_dmem() == 0);
	assert_true(gs_get_gpu_smem() == 2048);
	assert_calls(2, 1, 1, 2);
	assert_int_equal(log_count, 0);
}

static void strings_returned_unchanged(void **state)
{
	(void)state;
	static const char empty[] = "";
	char owned[] = "owned by the backend";
	const char *values[] = {NULL, empty, owned, "static renderer"};
	long allocs = bnum_allocs();

	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		driver_version = values[i];
		renderer = values[i];
		/* The very pointer comes back: no copy, no allocation. */
		assert_ptr_equal(gs_get_driver_version(), values[i]);
		assert_ptr_equal(gs_get_renderer(), values[i]);
		assert_calls(i + 1, i + 1, 0, 0);
	}
	assert_string_equal(owned, "owned by the backend");
	assert_true(bnum_allocs() == allocs);
	assert_int_equal(log_count, 0);
}

static void memory_values_bit_for_bit(void **state)
{
	(void)state;
	const uint64_t values[] = {0,
				   1,
				   UINT64_C(4294967295),
				   UINT64_C(4294967296),
				   UINT64_C(0x8000000000000000),
				   UINT64_C(0x0123456789abcdef),
				   UINT64_MAX};

	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		dmem = values[i];
		smem = ~values[i];
		assert_true(gs_get_gpu_dmem() == values[i]);
		assert_true(gs_get_gpu_smem() == ~values[i]);
		assert_calls(0, 0, i + 1, i + 1);
	}
	assert_int_equal(log_count, 0);
	assert_string_equal(message, "");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_context, setup, teardown),
		cmocka_unit_test_setup_teardown(absent_callbacks, setup, teardown),
		cmocka_unit_test_setup_teardown(each_slot_dispatches_alone, setup, teardown),
		cmocka_unit_test_setup_teardown(strings_returned_unchanged, setup, teardown),
		cmocka_unit_test_setup_teardown(memory_values_bit_for_bit, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
