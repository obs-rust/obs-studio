/* Regression test for obsproject/obs-studio#3427 (reported by micolous): an
 * Image source given a picture larger than the GPU can hold as a texture
 * (for example 20480x11520 when the limit is 16384) decodes the whole file,
 * keeps up to three full-size copies of it in memory and stalls the thread
 * that loads it for seconds, and only then does the texture creation fail.
 *
 * libobs must read the picture's size before it decodes the pixels, and give
 * up at once, with a log line that names the size and the limit, when the
 * graphics device cannot create a texture that big. Pictures up to the limit
 * must load as before.
 *
 * The test starts core video with the OpenGL module and finds the device's
 * texture size limit by creating 1-pixel-high textures. Then:
 *
 *   - a PNG one pixel wider than the limit and a few thousand rows high
 *     (hundreds of MiB once decoded) must fail to load, decode nothing (the
 *     peak resident memory of the process may grow by a few MiB at most) and
 *     log why;
 *   - the same for a narrow PNG one pixel taller than the limit;
 *   - a PNG exactly as wide as the limit, and a small one, must load and get
 *     a texture.
 *
 * The PNGs are written row by row with zlib, so making them costs no memory.
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <zlib.h>

#include <obs.h>
#include <graphics/image-file.h>
#include <util/dstr.h>
#include <util/platform.h>

#define BASE 64

static bool obs_up;
static uint32_t max_size;

/* ------------------------------------------------------------------------- */
/* log capture */

static struct dstr log_text;
static log_handler_t prev_handler;
static void *prev_param;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	va_list copy;
	va_copy(copy, args);
	char line[1024];
	vsnprintf(line, sizeof(line), format, copy);
	va_end(copy);
	dstr_cat(&log_text, line);
	dstr_cat(&log_text, "\n");
	if (prev_handler)
		prev_handler(level, format, args, prev_param);
}

/* ------------------------------------------------------------------------- */
/* PNG writer (RGB, 8 bits per channel), one row at a time */

static void put_be32(uint8_t *p, uint32_t v)
{
	p[0] = (uint8_t)(v >> 24);
	p[1] = (uint8_t)(v >> 16);
	p[2] = (uint8_t)(v >> 8);
	p[3] = (uint8_t)v;
}

static void write_chunk(FILE *f, const char *type, const uint8_t *data, uint32_t size)
{
	uint8_t be[4];
	put_be32(be, size);
	assert_int_equal(fwrite(be, 1, 4, f), 4);
	assert_int_equal(fwrite(type, 1, 4, f), 4);
	if (size)
		assert_int_equal(fwrite(data, 1, size, f), size);
	uLong crc = crc32(0L, (const Bytef *)type, 4);
	if (size)
		crc = crc32(crc, data, size);
	put_be32(be, (uint32_t)crc);
	assert_int_equal(fwrite(be, 1, 4, f), 4);
}

static void write_png(const char *path, uint32_t cx, uint32_t cy)
{
	FILE *f = fopen(path, "wb");
	assert_non_null(f);

	static const uint8_t sig[8] = {0x89, 'P', 'N', 'G', '\r', '\n', 0x1a, '\n'};
	assert_int_equal(fwrite(sig, 1, sizeof(sig), f), sizeof(sig));

	uint8_t ihdr[13] = {0};
	put_be32(ihdr, cx);
	put_be32(ihdr + 4, cy);
	ihdr[8] = 8; /* bit depth */
	ihdr[9] = 2; /* colour type: RGB */
	write_chunk(f, "IHDR", ihdr, sizeof(ihdr));

	const size_t row_size = 1 + (size_t)cx * 3;
	uint8_t *row = malloc(row_size);
	const size_t out_size = 1 << 20;
	uint8_t *out = malloc(out_size);
	assert_non_null(row);
	assert_non_null(out);

	z_stream z = {0};
	assert_int_equal(deflateInit(&z, 1), Z_OK);
	for (uint32_t y = 0; y <= cy; y++) {
		const bool last = y == cy;
		if (!last) {
			row[0] = 0; /* filter: none */
			for (size_t i = 1; i < row_size; i++)
				row[i] = (uint8_t)(i * 7 + y);
			z.next_in = row;
			z.avail_in = (uInt)row_size;
		}
		int ret;
		do {
			z.next_out = out;
			z.avail_out = (uInt)out_size;
			ret = deflate(&z, last ? Z_FINISH : Z_NO_FLUSH);
			assert_true(ret == Z_OK || ret == Z_STREAM_END || ret == Z_BUF_ERROR);
			const uint32_t have = (uint32_t)(out_size - z.avail_out);
			if (have)
				write_chunk(f, "IDAT", out, have);
		} while (z.avail_out == 0 || (last && ret != Z_STREAM_END));
	}
	deflateEnd(&z);
	write_chunk(f, "IEND", NULL, 0);

	free(out);
	free(row);
	assert_int_equal(fclose(f), 0);
}

/* ------------------------------------------------------------------------- */

static long peak_rss_kib(void)
{
	FILE *f = fopen("/proc/self/status", "r");
	if (!f)
		return -1;
	char line[256];
	long kib = -1;
	while (fgets(line, sizeof(line), f)) {
		if (strncmp(line, "VmHWM:", 6) == 0) {
			kib = strtol(line + 6, NULL, 10);
			break;
		}
	}
	fclose(f);
	return kib;
}

static bool texture_fits(uint32_t cx, uint32_t cy)
{
	obs_enter_graphics();
	gs_texture_t *tex = gs_texture_create(cx, cy, GS_BGRA, 1, NULL, 0);
	gs_texture_destroy(tex);
	obs_leave_graphics();
	return tex != NULL;
}

/* The largest texture width the device takes (up to 2^20). */
static uint32_t find_max_texture_size(void)
{
	uint32_t lo = 1, hi = 1;
	while (hi < (1u << 20) && texture_fits(hi * 2, 1))
		hi *= 2;
	lo = hi;
	hi = hi * 2;
	while (hi - lo > 1) {
		const uint32_t mid = lo + (hi - lo) / 2;
		if (texture_fits(mid, 1))
			lo = mid;
		else
			hi = mid;
	}
	return lo;
}

/* Load a cx x cy PNG through the image file API the Image source uses. */
static bool load_png(const char *path, uint32_t cx, uint32_t cy, long *peak_growth_mib, double *seconds)
{
	write_png(path, cx, cy);

	dstr_free(&log_text);
	const long peak_before = peak_rss_kib();
	const uint64_t t0 = os_gettime_ns();

	gs_image_file_ex_t image;
	gs_image_file_ex_init(&image, path, GS_IMAGE_ALPHA_STRAIGHT);

	*seconds = (double)(os_gettime_ns() - t0) / 1e9;
	*peak_growth_mib = (peak_rss_kib() - peak_before) / 1024;

	obs_enter_graphics();
	gs_image_file_ex_init_texture(&image);
	const bool ok = image.loaded && image.texture && image.cx == cx && image.cy == cy;
	gs_image_file_ex_free(&image);
	obs_leave_graphics();

	remove(path);
	return ok;
}

static void check_too_large(uint32_t cx, uint32_t cy)
{
	char path[64];
	snprintf(path, sizeof(path), "test_image_size_precheck_%ux%u.png", cx, cy);

	long growth;
	double seconds;
	const bool ok = load_png(path, cx, cy, &growth, &seconds);
	fprintf(stderr, "%ux%u (limit %u): loaded=%d in %.2f s, peak RSS +%ld MiB\n", cx, cy, max_size, ok, seconds,
		growth);

	assert_false(ok);
	/* Decoded, the picture is hundreds of MiB per copy. */
	if (growth > 32) {
		fprintf(stderr, "the oversized picture was decoded (peak RSS +%ld MiB)\n", growth);
		fail();
	}

	char size[64];
	snprintf(size, sizeof(size), "%ux%u", cx, cy);
	if (!strstr(log_text.array ? log_text.array : "", size) ||
	    !strstr(log_text.array ? log_text.array : "", "texture size")) {
		fprintf(stderr, "no log line names the size %s and the texture size limit\n", size);
		fail();
	}
}

/* Must run first: the peak RSS only grows. */
static void too_wide_fails_before_decoding(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	/* As many rows as FFmpeg's own size check lets through, up to 8192:
	 * 16385x8192 is 512 MiB as RGBA. */
	const uint32_t cx = max_size + 1;
	uint32_t cy = (uint32_t)(((int64_t)INT32_MAX / 8) / (cx + 128)) - 256;
	if (cy > 8192)
		cy = 8192;
	check_too_large(cx, cy);
}

static void too_tall_fails_before_decoding(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	check_too_large(16, max_size + 1);
}

static void at_limit_loads(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	long growth;
	double seconds;
	assert_true(load_png("test_image_size_precheck_limit.png", max_size, 16, &growth, &seconds));
	assert_true(load_png("test_image_size_precheck_limit.png", 16, max_size, &growth, &seconds));
}

static void small_loads(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	long growth;
	double seconds;
	assert_true(load_png("test_image_size_precheck_small.png", 64, 32, &growth, &seconds));
}

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = BASE,
		.base_height = BASE,
		.output_width = BASE,
		.output_height = BASE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	max_size = find_max_texture_size();
	fprintf(stderr, "device texture size limit: %u\n", max_size);

	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		base_set_log_handler(prev_handler, prev_param);
		obs_shutdown();
	}
	dstr_free(&log_text);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(too_wide_fails_before_decoding),
		cmocka_unit_test(too_tall_fails_before_decoding),
		cmocka_unit_test(at_limit_loads),
		cmocka_unit_test(small_loads),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
