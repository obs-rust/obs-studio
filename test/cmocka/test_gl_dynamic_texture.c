/* A dynamic texture's map, write and unmap (libobs-opengl gl-texture2d.c)
 * must hand the exact bytes to the next draw (#3191). gs_texture_unmap()
 * re-specified the texture with glTexImage2D on every unmap, which may
 * reallocate its storage each async frame. The storage is specified at
 * creation, so the unmap now updates it with glTexSubImage2D.
 *
 * Each case writes a different pattern on every frame, reads the texture
 * back through a stage surface and requires the exact bytes, then prints the
 * median time of a map, write and unmap. It runs libobs-opengl on whatever
 * GL the display has (Mesa's software GL under xvfb-run in CI) and skips
 * without a display. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/platform.h>

#define FRAMES 15

static bool obs_up;

static int setup(void **state)
{
	(void)state;
	const char *display = getenv("DISPLAY");
	if (!display || !*display)
		return 0;
	if (!obs_startup("en-US", NULL, NULL))
		return 0;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "core video failed; skipping\n");
		obs_shutdown();
		return 0;
	}
	obs_up = true;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

static uint8_t pattern(uint32_t frame, uint32_t x, uint32_t y, uint32_t c)
{
	return (uint8_t)(((x * 7 + y * 13 + c * 31) ^ (frame * 0x5b)) & 0xff);
}

static int cmp_u64(const void *a, const void *b)
{
	uint64_t x = *(const uint64_t *)a, y = *(const uint64_t *)b;
	return x < y ? -1 : x > y;
}

static void run(uint32_t width, uint32_t height, const char *label)
{
	if (!obs_up)
		skip();

	obs_enter_graphics();
	gs_texture_t *tex = gs_texture_create(width, height, GS_BGRA, 1, NULL, GS_DYNAMIC);
	gs_stagesurf_t *stage = gs_stagesurface_create(width, height, GS_BGRA);
	assert_non_null(tex);
	assert_non_null(stage);

	const size_t row_bytes = (size_t)width * 4;
	uint8_t *src = bmalloc(row_bytes * height);
	uint64_t t[FRAMES];
	for (uint32_t frame = 0; frame < FRAMES; frame++) {
		for (uint32_t y = 0; y < height; y++)
			for (uint32_t x = 0; x < width; x++)
				for (uint32_t c = 0; c < 4; c++)
					src[y * row_bytes + x * 4 + c] = pattern(frame, x, y, c);

		uint8_t *ptr;
		uint32_t linesize;
		uint64_t start = os_gettime_ns();
		assert_true(gs_texture_map(tex, &ptr, &linesize));
		for (uint32_t y = 0; y < height; y++)
			memcpy(ptr + (size_t)y * linesize, src + y * row_bytes, row_bytes);
		gs_texture_unmap(tex);
		gs_flush();
		t[frame] = os_gettime_ns() - start;

		gs_stage_texture(stage, tex);
		uint8_t *data;
		uint32_t stage_linesize;
		assert_true(gs_stagesurface_map(stage, &data, &stage_linesize));
		for (uint32_t y = 0; y < height; y++) {
			const uint8_t *row = data + (size_t)y * stage_linesize;
			for (uint32_t x = 0; x < width; x++)
				for (uint32_t c = 0; c < 4; c++)
					if (row[x * 4 + c] != pattern(frame, x, y, c))
						fail_msg("frame %u: pixel (%u, %u) channel %u is %u, expected %u",
							 frame, x, y, c, row[x * 4 + c], pattern(frame, x, y, c));
		}
		gs_stagesurface_unmap(stage);
	}

	bfree(src);
	gs_stagesurface_destroy(stage);
	gs_texture_destroy(tex);
	obs_leave_graphics();

	qsort(t, FRAMES, sizeof(t[0]), cmp_u64);
	printf("%s: map, write and unmap %.3f ms (median of %d)\n", label, (double)t[FRAMES / 2] / 1e6, FRAMES);
}

static void dynamic_texture_1080p(void **state)
{
	(void)state;
	run(1920, 1080, "1080p BGRA");
}

static void dynamic_texture_4k(void **state)
{
	(void)state;
	run(3840, 2160, "4K BGRA");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(dynamic_texture_1080p),
		cmocka_unit_test(dynamic_texture_4k),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
