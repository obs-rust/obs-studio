/* Actual-library coverage for the remaining texture-unit cases in #4705.
 * The hybrid intentionally rejects nonzero immediate units; the C reference
 * appends to in-bounds units. Never send out-of-bounds indices to the reference.
 * CTest selects the library and expected mode independently of its behavior. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics-internal.h"
#include "graphics/graphics-immediate.h"
#include "util/base.h"

static struct graphics_subsystem graphics;
static bool hybrid;
static unsigned logs;
static log_handler_t saved_handler;
static void *saved_param;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	logs++;
}

static void context(gs_device_t *device)
{
	assert_null(device);
}

static uint32_t bits(float value)
{
	uint32_t result;
	memcpy(&result, &value, sizeof(result));
	return result;
}

static void coordinate(const struct vec2 *value, float x, float y)
{
	assert_int_equal(bits(value->x), bits(x));
	assert_int_equal(bits(value->y), bits(y));
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	logs = 0;
	gs_enter_context(&graphics);
	/* Seed every array through the actual public adapter. A rejected call must
	 * preserve existing data as well as refusing an append to an empty array. */
	for (int unit = 0; unit < GS_IMMEDIATE_TEX_ARRAYS; unit++) {
		gs_texcoord((float)unit, -0.0f, unit);
		assert_int_equal(graphics.texverts[unit].num, 1);
		coordinate(&graphics.texverts[unit].array[0], (float)unit, -0.0f);
	}
	assert_int_equal(logs, 0);
	graphics.using_immediate = true;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	/* The fixture never creates a render batch or transfers array ownership. */
	for (unsigned unit = 0; unit < GS_IMMEDIATE_TEX_ARRAYS; unit++)
		da_free(graphics.texverts[unit]);
	da_free(graphics.verts);
	da_free(graphics.norms);
	da_free(graphics.colors);
	base_set_log_handler(saved_handler, saved_param);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void rejected(int unit)
{
	struct vec2 *arrays[GS_IMMEDIATE_TEX_ARRAYS];
	size_t capacities[GS_IMMEDIATE_TEX_ARRAYS];
	for (unsigned i = 0; i < GS_IMMEDIATE_TEX_ARRAYS; i++) {
		arrays[i] = graphics.texverts[i].array;
		capacities[i] = graphics.texverts[i].capacity;
	}
	gs_texcoord(-0.0f, 2.5f, unit);
	for (unsigned i = 0; i < GS_IMMEDIATE_TEX_ARRAYS; i++) {
		assert_int_equal(graphics.texverts[i].num, 1);
		assert_int_equal(graphics.texverts[i].capacity, capacities[i]);
		assert_ptr_equal(graphics.texverts[i].array, arrays[i]);
		coordinate(&graphics.texverts[i].array[0], (float)i, -0.0f);
	}
	assert_int_equal(graphics.verts.num, 0);
	assert_int_equal(graphics.norms.num, 0);
	assert_int_equal(graphics.colors.num, 0);
	assert_int_equal(logs, 0);
}

static void unit_zero_appends_once(void **state)
{
	(void)state;
	gs_texcoord(-0.0f, 2.5f, 0);
	for (unsigned unit = 0; unit < GS_IMMEDIATE_TEX_ARRAYS; unit++) {
		assert_int_equal(graphics.texverts[unit].num, unit == 0 ? 2 : 1);
		coordinate(&graphics.texverts[unit].array[0], (float)unit, -0.0f);
	}
	coordinate(&graphics.texverts[0].array[1], -0.0f, 2.5f);
	assert_int_equal(logs, 0);
}

static void nonzero_immediate_units(void **state)
{
	(void)state;
	for (int unit = 1; unit < GS_IMMEDIATE_TEX_ARRAYS; unit++) {
		if (hybrid) {
			rejected(unit);
		} else {
			/* Characterized, not endorsed: the original C permits an append
			 * to each in-bounds nonzero immediate unit. Teardown frees it. */
			gs_texcoord(-0.0f, 2.5f, unit);
			for (int i = 0; i < GS_IMMEDIATE_TEX_ARRAYS; i++) {
				assert_int_equal(graphics.texverts[i].num, i > 0 && i <= unit ? 2 : 1);
				coordinate(&graphics.texverts[i].array[0], (float)i, -0.0f);
				if (i > 0 && i <= unit)
					coordinate(&graphics.texverts[i].array[1], -0.0f, 2.5f);
			}
		}
	}
	/* Rejection must not poison the next supported call. */
	gs_texcoord(-0.0f, 2.5f, 0);
	assert_int_equal(graphics.texverts[0].num, 2);
	coordinate(&graphics.texverts[0].array[1], -0.0f, 2.5f);
	assert_int_equal(logs, 0);
}

static void out_of_bounds_units(void **state)
{
	(void)state;
	const int units[] = {INT_MIN, -1, GS_IMMEDIATE_TEX_ARRAYS, INT_MAX};
	/* Hybrid only: the C reference would index outside texverts. Check both
	 * modes so the non-immediate path cannot accidentally lose its bounds. */
	for (unsigned mode = 0; mode < 2; mode++) {
		graphics.using_immediate = mode == 0;
		for (unsigned i = 0; i < sizeof(units) / sizeof(units[0]); i++)
			rejected(units[i]);
	}
	graphics.using_immediate = true;
	unit_zero_appends_once(state);
}

int main(int argc, char **argv)
{
	if (argc != 2 || (strcmp(argv[1], "hybrid") != 0 && strcmp(argv[1], "reference") != 0)) {
		fprintf(stderr, "usage: %s hybrid|reference\n", argv[0]);
		return 2;
	}
	hybrid = strcmp(argv[1], "hybrid") == 0;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(unit_zero_appends_once, setup, teardown),
		cmocka_unit_test_setup_teardown(nonzero_immediate_units, setup, teardown),
		cmocka_unit_test_setup_teardown(out_of_bounds_units, setup, teardown),
	};
	return _cmocka_run_group_tests(hybrid ? "hybrid" : "reference", tests, hybrid ? 3 : 2, NULL, NULL);
}
