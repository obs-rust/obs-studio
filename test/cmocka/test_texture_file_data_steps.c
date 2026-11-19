/* Rust-export recording characterization. The real private C adapter retains
 * FFmpeg layout and callback plumbing; the orchestration comes from Rust. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "libobs/build/libobs-media.pch.h"

static const char missing[] = "legacy-missing.png";
static const char corrupt[] = "legacy-corrupt.png";
static struct ffmpeg_image *session;
static enum gs_color_format *format_out;
static uint32_t *width_out, *height_out;
static unsigned calls;
static bool init_ok, decode_ok;
static uint8_t *pixels;
static const uint8_t expected_pixels[] = {1, 2, 3, 128, 4, 5, 6, 0};

static uint32_t record_max_size(void)
{
	return 4096;
}

static bool record_size(struct ffmpeg_image *image, int cx, int cy)
{
	(void)image;
	(void)cx;
	(void)cy;
	/* Only callback identity matters to the wrapper. */
	assert_true(false);
	return false;
}

static void *record_convert(struct ffmpeg_image *image, AVFrame *frame, enum gs_image_alpha_mode alpha)
{
	(void)image;
	(void)frame;
	(void)alpha;
	assert_true(false);
	return NULL;
}

static bool record_init(struct ffmpeg_image *image, const char *file, uint32_t (*max_size)(void))
{
	assert_int_equal(calls++, 0);
	assert_ptr_equal(file, init_ok ? corrupt : missing);
	assert_ptr_equal(max_size, record_max_size);
	session = image;
	memset(image, 0, sizeof(*image));
	image->format = AV_PIX_FMT_BGRA;
	image->cx = 71;
	image->cy = 72;
	return init_ok;
}

static void *record_decode(void *context, int alpha)
{
	assert_int_equal(calls++, 1);
	struct gs_image_decode_context *decode = context;
	assert_ptr_equal(decode->image, session);
	assert_ptr_equal(decode->check_size, record_size);
	assert_ptr_equal(decode->convert, record_convert);
	assert_int_equal(alpha, GS_IMAGE_ALPHA_STRAIGHT);
	assert_int_equal(*format_out, GS_BGRX);
	assert_int_equal(*width_out, 91);
	assert_int_equal(*height_out, 92);
	/* Decode can change the image's dimensions/format (e.g. orientation). */
	session->format = AV_PIX_FMT_RGBA;
	session->cx = 2;
	session->cy = 1;
	if (!decode_ok)
		return NULL;
	pixels = bmemdup(expected_pixels, sizeof(expected_pixels));
	return pixels;
}

static int record_format(int format)
{
	assert_int_equal(calls++, 2);
	assert_int_equal(format, AV_PIX_FMT_RGBA);
	return GS_RGBA;
}

static void record_free(struct ffmpeg_image *image)
{
	assert_ptr_equal(image, session);
	assert_int_equal(calls++, decode_ok ? 3 : 2);
	/* Pin publication before cleanup; failure must not write outputs. */
	assert_int_equal(*format_out, decode_ok ? GS_RGBA : GS_BGRX);
	assert_int_equal(*width_out, decode_ok ? 2 : 91);
	assert_int_equal(*height_out, decode_ok ? 1 : 92);
	memset(image, 0, sizeof(*image));
}

#define gs_create_texture_file_data record_texture_file_data
#define ffmpeg_image_init record_init
#define ffmpeg_image_free record_free
#define gs_ffmpeg_image_decode record_decode
#define gs_ffmpeg_image_color_format record_format
#define gs_image_max_texture_size record_max_size
#define ffmpeg_image_too_large record_size
#define ffmpeg_image_reformat_frame record_convert
#include "libobs/graphics/graphics-texture-file-data-adapter.c.h"
#undef ffmpeg_image_reformat_frame
#undef ffmpeg_image_too_large
#undef gs_image_max_texture_size
#undef gs_ffmpeg_image_color_format
#undef gs_ffmpeg_image_decode
#undef ffmpeg_image_free
#undef ffmpeg_image_init
#undef gs_create_texture_file_data
#define record_texture_file_data gs_create_texture_file_data

static void check_load(bool initialized, bool decoded)
{
	const long allocations = bnum_allocs();
	enum gs_color_format format = GS_BGRX;
	uint32_t width = 91, height = 92;
	format_out = &format;
	width_out = &width;
	height_out = &height;
	calls = 0;
	init_ok = initialized;
	decode_ok = decoded;
	pixels = NULL;
	uint8_t *data = record_texture_file_data(initialized ? corrupt : missing, &format, &width, &height);
	assert_int_equal(calls, !initialized ? 1 : decoded ? 4 : 3);
	if (initialized && decoded) {
		assert_ptr_equal(data, pixels);
		assert_memory_equal(data, expected_pixels, sizeof(expected_pixels));
		assert_int_equal(format, GS_RGBA);
		assert_int_equal(width, 2);
		assert_int_equal(height, 1);
		/* The caller owns the decode allocation after session cleanup. */
		assert_int_equal(bnum_allocs(), allocations + 1);
		bfree(data);
	} else {
		assert_null(data);
		assert_int_equal(format, GS_BGRX);
		assert_int_equal(width, 91);
		assert_int_equal(height, 92);
	}
	assert_int_equal(bnum_allocs(), allocations);
}

static void failed_init_has_no_decode_or_second_free(void **state)
{
	(void)state;
	check_load(false, false);
}

static void failed_decode_frees_without_output_writes(void **state)
{
	(void)state;
	check_load(true, false);
}

static void successful_decode_publishes_before_free(void **state)
{
	(void)state;
	check_load(true, true);
}

static void repeated_failure_and_success(void **state)
{
	(void)state;
	for (unsigned i = 0; i < 8; i++) {
		check_load(false, false);
		check_load(true, false);
		check_load(true, true);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(failed_init_has_no_decode_or_second_free),
					   cmocka_unit_test(failed_decode_frees_without_output_writes),
					   cmocka_unit_test(successful_decode_publishes_before_free),
					   cmocka_unit_test(repeated_failure_and_success)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

#undef record_texture_file_data
