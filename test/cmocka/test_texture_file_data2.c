#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <cmocka.h>

#include <graphics/graphics.h>
#include <util/bmem.h>

/* Public API characterization: the legacy loader discards only the color
 * space output of data3. No OBS core, graphics device or texture is created. */
static const enum gs_image_alpha_mode alpha_modes[] = {
	GS_IMAGE_ALPHA_STRAIGHT,
	GS_IMAGE_ALPHA_PREMULTIPLY_SRGB,
	GS_IMAGE_ALPHA_PREMULTIPLY,
};

static const uint8_t straight_pixels[] = {255, 0,  0,  255, 0,  255, 0,  128, 0,  0,  255, 255,
					  10,  20, 30, 255, 40, 50,  60, 0,   70, 80, 90,  255};

static void check_valid(enum gs_image_alpha_mode mode)
{
	const long allocs = bnum_allocs();
	enum gs_color_format format2 = GS_BGRA, format3 = GS_BGRX;
	enum gs_color_space space = GS_CS_709_EXTENDED;
	uint32_t width2 = 91, height2 = 92, width3 = 93, height3 = 94;
	const char *path = TEXTURE_DATA2_DATA_DIR "/image-file.png";
	uint8_t *data2 = gs_create_texture_file_data2(path, mode, &format2, &width2, &height2);
	uint8_t *data3 = gs_create_texture_file_data3(path, mode, &format3, &width3, &height3, &space);
	assert_non_null(data2);
	assert_non_null(data3);
	assert_ptr_not_equal(data2, data3);
	assert_int_equal(format2, GS_RGBA);
	assert_int_equal(width2, 3);
	assert_int_equal(height2, 2);
	assert_int_equal(format2, format3);
	assert_int_equal(width2, width3);
	assert_int_equal(height2, height3);
	assert_int_equal(space, GS_CS_SRGB);
	assert_memory_equal(data2, data3, sizeof(straight_pixels));
	if (mode == GS_IMAGE_ALPHA_STRAIGHT) {
		assert_memory_equal(data2, straight_pixels, sizeof(straight_pixels));
	} else {
		/* Both premultiplication modes preserve alpha and clear RGB at zero
		 * alpha. This also distinguishes them from straight-alpha routing. */
		for (size_t i = 3; i < sizeof(straight_pixels); i += 4)
			assert_int_equal(data2[i], straight_pixels[i]);
		assert_int_equal(data2[16], 0);
		assert_int_equal(data2[17], 0);
		assert_int_equal(data2[18], 0);
		assert_true(data2[5] < straight_pixels[5]);
	}
	bfree(data2);
	bfree(data3);
	assert_int_equal(bnum_allocs(), allocs);
}

static void check_failure(const char *path, enum gs_image_alpha_mode mode)
{
	const long allocs = bnum_allocs();
	enum gs_color_format format2 = GS_BGRA, format3 = GS_BGRA;
	enum gs_color_space space = GS_CS_709_EXTENDED;
	uint32_t width2 = 91, height2 = 92, width3 = 91, height3 = 92;
	uint8_t *data2 = gs_create_texture_file_data2(path, mode, &format2, &width2, &height2);
	uint8_t *data3 = gs_create_texture_file_data3(path, mode, &format3, &width3, &height3, &space);
	assert_null(data2);
	assert_null(data3);
	/* Characterized, not endorsed: a failed load leaves caller outputs alone. */
	assert_int_equal(format2, GS_BGRA);
	assert_int_equal(width2, 91);
	assert_int_equal(height2, 92);
	assert_int_equal(format2, format3);
	assert_int_equal(width2, width3);
	assert_int_equal(height2, height3);
	assert_int_equal(space, GS_CS_709_EXTENDED);
	bfree(data2);
	bfree(data3);
	assert_int_equal(bnum_allocs(), allocs);
}

static void valid_png_all_alpha_modes(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(alpha_modes) / sizeof(alpha_modes[0]); i++)
		check_valid(alpha_modes[i]);
}

static void failed_loads_preserve_outputs(void **state)
{
	(void)state;
	const char *paths[] = {"", TEXTURE_DATA2_DATA_DIR "/texture-data2-does-not-exist.png", TEXTURE_DATA2_CORRUPT};
	for (size_t i = 0; i < sizeof(alpha_modes) / sizeof(alpha_modes[0]); i++)
		for (size_t j = 0; j < sizeof(paths) / sizeof(paths[0]); j++)
			check_failure(paths[j], alpha_modes[i]);
}

static void repeated_failure_and_success(void **state)
{
	for (int i = 0; i < 8; i++) {
		failed_loads_preserve_outputs(state);
		valid_png_all_alpha_modes(state);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(valid_png_all_alpha_modes),
					   cmocka_unit_test(failed_loads_preserve_outputs),
					   cmocka_unit_test(repeated_failure_and_success)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
