#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/image-file.h>
#include <util/bmem.h>

/* Public-API characterization of FFmpeg decoder initialization and cleanup.
 * No graphics device is needed: the decoded CPU pixels are freed before any
 * texture is created. These assertions apply to both libobs implementations.
 * Allocation failure injection and FFmpeg-private handles are intentionally
 * outside this public-path characterization. */
static const uint8_t png_pixels[] = {255, 0,  0,  255, 0,  255, 0,  128, 0,  0,  255, 255,
				     10,  20, 30, 255, 40, 50,  60, 0,   70, 80, 90,  255};

static void check_load(const char *path, bool valid)
{
	const long allocs = bnum_allocs();
	enum gs_color_format format = GS_BGRA;
	uint32_t cx = 91, cy = 92;
	uint8_t *pixels = gs_create_texture_file_data(path, &format, &cx, &cy);
	if (valid) {
		assert_non_null(pixels);
		assert_int_equal(format, GS_RGBA);
		assert_int_equal(cx, 3);
		assert_int_equal(cy, 2);
		assert_memory_equal(pixels, png_pixels, sizeof(png_pixels));
	} else {
		assert_null(pixels);
		/* Characterized, not endorsed: failure preserves caller outputs. */
		assert_int_equal(format, GS_BGRA);
		assert_int_equal(cx, 91);
		assert_int_equal(cy, 92);
	}
	bfree(pixels);
	assert_int_equal(bnum_allocs(), allocs);

	gs_image_file_ex_t image;
	memset(&image, 0xa5, sizeof(image));
	gs_image_file_ex_init(&image, path, GS_IMAGE_ALPHA_STRAIGHT);
	assert_int_equal(image.loaded, valid);
	assert_false(image.is_animated_gif);
	assert_null(image.texture);
	if (valid) {
		assert_int_equal(image.cx, 3);
		assert_int_equal(image.cy, 2);
		assert_int_equal(image.format, GS_RGBA);
		assert_int_equal(image.mem_usage, sizeof(png_pixels));
		assert_non_null(image.texture_data);
		assert_memory_equal(image.texture_data, png_pixels, sizeof(png_pixels));
	} else {
		assert_int_equal(image.cx, 0);
		assert_int_equal(image.cy, 0);
		assert_int_equal(image.mem_usage, 0);
		assert_null(image.texture_data);
		assert_null(image.internal);
	}
	gs_image_file_ex_free(&image);
	assert_false(image.loaded);
	assert_null(image.texture_data);
	assert_null(image.internal);
	gs_image_file_ex_free(&image);
	assert_int_equal(bnum_allocs(), allocs);
}

static void empty_path(void **state)
{
	(void)state;
	check_load("", false);
}

static void missing_file(void **state)
{
	(void)state;
	check_load(FFMPEG_IMAGE_DATA_DIR "/ffmpeg-image-init-does-not-exist.png", false);
}

static void valid_png(void **state)
{
	(void)state;
	check_load(FFMPEG_IMAGE_DATA_DIR "/image-file.png", true);
}

static void corrupt_file(void **state)
{
	(void)state;
	check_load(FFMPEG_IMAGE_CORRUPT, false);
}

static void repeated_failure_and_success(void **state)
{
	for (int i = 0; i < 8; i++) {
		empty_path(state);
		missing_file(state);
		corrupt_file(state);
		valid_png(state);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(empty_path), cmocka_unit_test(missing_file),
					   cmocka_unit_test(valid_png), cmocka_unit_test(corrupt_file),
					   cmocka_unit_test(repeated_failure_and_success)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
