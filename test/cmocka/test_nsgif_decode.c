/*
 * The vendored libnsgif (libobs/graphics/libnsgif), decoded directly: a
 * one-frame GIF goes to FFmpeg in gs_image_file, so test_image_file does not
 * reach libnsgif for it. Each case decodes a GIF and compares frame 0 with
 * the RGB values of the reference image next to it (plain PPM, P3), as
 * libnsgif's own test suite does.
 *
 * libnsgif-lzweod.gif and .ppm are libnsgif's test/data/lzweod.gif and .ppm
 * (netsurf-browser/libnsgif 9abf1190d, MIT, see
 * libobs/graphics/libnsgif/LICENSE.libnsgif): the last LZW code ends on a
 * byte boundary at the end of the data, which the code reader's slow path
 * lost before upstream 2255b2f74 (obs-rust/obs-studio#2465).
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _MSC_VER
#pragma warning(push)
#pragma warning(disable : 4244 4267)
#endif
#include "graphics/libnsgif/lzw.c.h"
#include "graphics/libnsgif/gif.c.h"
#ifdef _MSC_VER
#pragma warning(pop)
#endif

#define DATA(name) NSGIF_DATA_DIR "/" name

static nsgif_bitmap_t *bitmap_create(int width, int height)
{
	return calloc((size_t)width * (size_t)height, 4);
}

static void bitmap_destroy(nsgif_bitmap_t *bitmap)
{
	free(bitmap);
}

static uint8_t *bitmap_get_buffer(nsgif_bitmap_t *bitmap)
{
	return (uint8_t *)bitmap;
}

static const nsgif_bitmap_cb_vt bitmap_vt = {
	.create = bitmap_create,
	.destroy = bitmap_destroy,
	.get_buffer = bitmap_get_buffer,
};

static uint8_t *read_file(const char *path, size_t *size)
{
	FILE *f = fopen(path, "rb");
	assert_non_null(f);
	assert_int_equal(fseek(f, 0, SEEK_END), 0);
	long length = ftell(f);
	assert_true(length > 0);
	assert_int_equal(fseek(f, 0, SEEK_SET), 0);
	uint8_t *data = malloc((size_t)length);
	assert_non_null(data);
	assert_int_equal(fread(data, 1, (size_t)length, f), (size_t)length);
	fclose(f);
	*size = (size_t)length;
	return data;
}

/* The P3 reference: its width, height and the RGB values of frame 0. Lines
 * starting with '#' are comments. */
static unsigned *read_ppm(const char *path, unsigned *width, unsigned *height)
{
	size_t size;
	char *text = (char *)read_file(path, &size);
	char *copy = calloc(size + 1, 1);
	assert_non_null(copy);
	size_t n = 0;
	for (size_t i = 0; i < size; i++) {
		if (text[i] == '#') {
			while (i < size && text[i] != '\n')
				i++;
		}
		if (i < size)
			copy[n++] = text[i];
	}
	free(text);

	unsigned max;
	int used;
	assert_int_equal(sscanf(copy, "P3 %u %u %u%n", width, height, &max, &used), 3);
	/* libnsgif writes its references with a maximum of 256. */
	assert_true(max == 255 || max == 256);
	size_t count = (size_t)*width * *height * 3;
	unsigned *rgb = calloc(count, sizeof(*rgb));
	assert_non_null(rgb);
	const char *p = copy + used;
	for (size_t i = 0; i < count; i++) {
		int step;
		assert_int_equal(sscanf(p, "%u%n", &rgb[i], &step), 1);
		p += step;
	}
	free(copy);
	return rgb;
}

static void assert_frame0_matches(const char *gif_path, const char *ppm_path)
{
	size_t size;
	uint8_t *data = read_file(gif_path, &size);
	unsigned width, height;
	unsigned *expected = read_ppm(ppm_path, &width, &height);

	nsgif_t *gif;
	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	assert_int_equal(nsgif_data_scan(gif, size, data), NSGIF_OK);
	nsgif_data_complete(gif);
	const nsgif_info_t *info = nsgif_get_info(gif);
	assert_int_equal(info->width, width);
	assert_int_equal(info->height, height);

	nsgif_bitmap_t *bitmap = NULL;
	assert_int_equal(nsgif_frame_decode(gif, 0, &bitmap), NSGIF_OK);
	assert_non_null(bitmap);
	const uint8_t *pixels = bitmap_get_buffer(bitmap);
	for (size_t px = 0; px < (size_t)width * height; px++) {
		for (size_t c = 0; c < 3; c++) {
			if (pixels[4 * px + c] != expected[3 * px + c]) {
				fprintf(stderr, "pixel %u channel %u: %u, expected %u\n", (unsigned)px, (unsigned)c,
					pixels[4 * px + c], expected[3 * px + c]);
				/* fail_msg() does not link on Windows (ci/test_cmocka_windows_diagnostics.py). */
				fail();
			}
		}
	}

	nsgif_destroy(gif);
	free(expected);
	free(data);
}

/* The last code ends on a byte boundary at the end of the data: all three
 * pixels, the last one blue. */
static void lzw_code_on_byte_boundary_at_end_of_data(void **state)
{
	(void)state;
	assert_frame0_matches(DATA("libnsgif-lzweod.gif"), DATA("libnsgif-lzweod.ppm"));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(lzw_code_on_byte_boundary_at_end_of_data),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
