/* C-source characterization of obs_canvas_has_valid_video_info (#5295).
 * Real private layout; no core startup, graphics, locks or own threads.
 * NULL is deliberately untested: the original C dereferences the canvas.
 * The real C rules are compiled too: their symbols are hidden in libobs.
 * This source-only fixture does not exercise a future Rust swap.
 * External mode lets the later Rust swap reuse these unchanged assertions. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <limits.h>
#include <string.h>
#include "obs-internal.h"

#ifndef CANVAS_VALID_INFO_EXTERNAL
#include "libobs/obs-canvas-has-valid-video-info.c.h"
#endif

static struct obs_canvas valid_canvas(void)
{
	struct obs_canvas canvas;
	memset(&canvas, 0xa5, sizeof(canvas));
	canvas.ovi = (struct obs_video_info){
		.base_width = 1920,
		.base_height = 1080,
		.output_width = 1280,
		.output_height = 720,
		.output_format = VIDEO_FORMAT_I420,
	};
	return canvas;
}

static void check_validity(struct obs_canvas *canvas, bool expected)
{
	unsigned char before[sizeof(*canvas)];
	memcpy(before, canvas, sizeof(before));
	long allocations = bnum_allocs();
	assert_int_equal(obs_canvas_has_valid_video_info(canvas), expected);
	assert_memory_equal(canvas, before, sizeof(before));
	assert_int_equal(bnum_allocs(), allocations);
}

static void dimension_boundaries(size_t offset)
{
	static const struct {
		uint32_t value;
		bool expected;
	} cases[] = {{0, false}, {1, true}, {2, true}, {UINT32_MAX / 2, true}, {UINT32_MAX, true}};
	struct obs_canvas canvas = valid_canvas();
	uint32_t *dimension = (uint32_t *)((unsigned char *)&canvas.ovi + offset);
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		*dimension = cases[i].value;
		check_validity(&canvas, cases[i].expected);
	}
}

static void base_width(void **state)
{
	(void)state;
	dimension_boundaries(offsetof(struct obs_video_info, base_width));
}

static void base_height(void **state)
{
	(void)state;
	dimension_boundaries(offsetof(struct obs_video_info, base_height));
}

static void output_width(void **state)
{
	(void)state;
	dimension_boundaries(offsetof(struct obs_video_info, output_width));
}

static void output_height(void **state)
{
	(void)state;
	dimension_boundaries(offsetof(struct obs_video_info, output_height));
}

static void output_format(void **state)
{
	(void)state;
	static const struct {
		enum video_format value;
		bool expected;
	} cases[] = {{VIDEO_FORMAT_NONE, false},      {VIDEO_FORMAT_I420, true},
		     {VIDEO_FORMAT_RGBA, true},       {(enum video_format)INT_MIN, true},
		     {(enum video_format) - 1, true}, {(enum video_format)INT_MAX, true}};
	struct obs_canvas canvas = valid_canvas();
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		/* Out-of-enum formats pass today: characterized, not endorsed. */
		canvas.ovi.output_format = cases[i].value;
		check_validity(&canvas, cases[i].expected);
	}
}

static void conjunction(void **state)
{
	(void)state;
	for (unsigned mask = 0; mask < 32; mask++) {
		struct obs_canvas canvas = valid_canvas();
		canvas.ovi.base_width = (mask & 1) ? 1 : 0;
		canvas.ovi.base_height = (mask & 2) ? 1 : 0;
		canvas.ovi.output_width = (mask & 4) ? 1 : 0;
		canvas.ovi.output_height = (mask & 8) ? 1 : 0;
		canvas.ovi.output_format = (mask & 16) ? VIDEO_FORMAT_I420 : VIDEO_FORMAT_NONE;
		check_validity(&canvas, mask == 31);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(base_width),    cmocka_unit_test(base_height),
					   cmocka_unit_test(output_width),  cmocka_unit_test(output_height),
					   cmocka_unit_test(output_format), cmocka_unit_test(conjunction)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
