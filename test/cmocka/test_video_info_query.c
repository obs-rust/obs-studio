/* C-source characterization for #5167. The real-header fixture compiles the
 * getter itself, without a GPU or a running core. test_no_core_getters keeps
 * checking the actual shared-library entry point. The later Rust swap must
 * relink this unchanged fixture to Rust and its real private field adapters. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"

struct obs_core *obs;
static struct obs_core core;
static struct obs_canvas canvas;
static struct obs_core_video_mix mix;
static char graphics_token;
static const char module_name[] = "borrowed graphics module";

static int setup(void **state)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	memset(&canvas, 0, sizeof(canvas));
	memset(&mix, 0, sizeof(mix));
	obs = &core;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs = NULL;
	return 0;
}

static void unavailable(void)
{
	struct obs_video_info output, before;
	memset(&output, 0xa5, sizeof(output));
	memcpy(&before, &output, sizeof(before));
	assert_false(obs_get_video_info(&output));
	assert_memory_equal(&output, &before, sizeof(output));
	assert_false(obs_get_video_info(NULL));
}

static void no_core(void **state)
{
	(void)state;
	obs = NULL;
	unavailable();
}

static void no_graphics(void **state)
{
	(void)state;
	/* A NULL main_canvas is safe because graphics is checked first. */
	unavailable();
	core.data.main_canvas = &canvas;
	canvas.mix = &mix;
	unavailable();
}

static void no_mix(void **state)
{
	(void)state;
	core.video.graphics = (graphics_t *)&graphics_token;
	core.data.main_canvas = &canvas;
	unavailable();
}

static void complete_copy(void **state)
{
	(void)state;
	core.video.graphics = (graphics_t *)&graphics_token;
	core.data.main_canvas = &canvas;
	canvas.mix = &mix;
	for (unsigned i = 0; i < 2; i++) {
		/* Characterized, not endorsed: zero dimensions/FPS denominator and
		 * extreme integers are copied without validation or normalization.
		 * A NULL mix.video is irrelevant; live video requires an output. */
		const struct obs_video_info stored = {
			.graphics_module = i ? NULL : module_name,
			.fps_num = i ? 0 : UINT32_MAX,
			.fps_den = i ? UINT32_MAX : 0,
			.base_width = i ? 1 : 0,
			.base_height = 7919,
			.output_width = UINT32_MAX,
			.output_height = 101,
			.output_format = i ? VIDEO_FORMAT_RGBA : VIDEO_FORMAT_NV12,
			.adapter = 0x12345678,
			.gpu_conversion = i != 0,
			.colorspace = i ? VIDEO_CS_709 : VIDEO_CS_SRGB,
			.range = i ? VIDEO_RANGE_FULL : VIDEO_RANGE_PARTIAL,
			.scale_type = i ? OBS_SCALE_POINT : OBS_SCALE_LANCZOS,
		};
		mix.ovi = stored;
		unsigned char before[sizeof(mix.ovi)];
		memcpy(before, &mix.ovi, sizeof(before));
		struct obs_video_info output;
		memset(&output, 0x5a, sizeof(output));
		assert_true(obs_get_video_info(&output));
		assert_ptr_equal(output.graphics_module, stored.graphics_module);
		assert_int_equal(output.fps_num, stored.fps_num);
		assert_int_equal(output.fps_den, stored.fps_den);
		assert_int_equal(output.base_width, stored.base_width);
		assert_int_equal(output.base_height, stored.base_height);
		assert_int_equal(output.output_width, stored.output_width);
		assert_int_equal(output.output_height, stored.output_height);
		assert_int_equal(output.output_format, stored.output_format);
		assert_int_equal(output.adapter, stored.adapter);
		assert_int_equal(output.gpu_conversion, stored.gpu_conversion);
		assert_int_equal(output.colorspace, stored.colorspace);
		assert_int_equal(output.range, stored.range);
		assert_int_equal(output.scale_type, stored.scale_type);
		assert_memory_equal(&mix.ovi, before, sizeof(before));
		assert_ptr_equal(obs, &core);
		assert_ptr_equal(core.video.graphics, &graphics_token);
		assert_ptr_equal(core.data.main_canvas, &canvas);
		assert_ptr_equal(canvas.mix, &mix);
		assert_null(mix.video);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(no_core, setup, teardown),
					   cmocka_unit_test_setup_teardown(no_graphics, setup, teardown),
					   cmocka_unit_test_setup_teardown(no_mix, setup, teardown),
					   cmocka_unit_test_setup_teardown(complete_copy, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
