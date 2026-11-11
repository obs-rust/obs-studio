#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include "libobs/build/libobs.pch.h"

/* Direct C-source characterization, not a hybrid/Rust-symbol test. The
 * unchanged real-source deinterlace tests cover integration. Keep this file
 * unchanged when porting its assertions to Rust. No graphics context needed. */
static struct {
	uint32_t width, height, levels, flags;
	enum gs_color_format format;
	const uint8_t **data;
} allocations[MAX_AV_PLANES];
static unsigned texture_calls, render_calls;
static enum gs_color_format render_format;
static enum gs_zstencil_format render_zs;
static int fail_plane;
static bool fail_render;
static unsigned char handles[MAX_AV_PLANES], render_handle;

static gs_texrender_t *record_texrender(enum gs_color_format format, enum gs_zstencil_format zs)
{
	assert_int_equal(texture_calls, 0);
	assert_int_equal(render_calls++, 0);
	render_format = format;
	render_zs = zs;
	return fail_render ? NULL : (gs_texrender_t *)&render_handle;
}

static gs_texture_t *record_texture(uint32_t width, uint32_t height, enum gs_color_format format, uint32_t levels,
				    const uint8_t **data, uint32_t flags)
{
	unsigned c = texture_calls++;
	assert_true(c < MAX_AV_PLANES);
	allocations[c].width = width;
	allocations[c].height = height;
	allocations[c].format = format;
	allocations[c].levels = levels;
	allocations[c].data = data;
	allocations[c].flags = flags;
	return (int)c == fail_plane ? NULL : (gs_texture_t *)&handles[c];
}

#define gs_texrender_create record_texrender
#define gs_texture_create record_texture
#include "libobs/obs-source-deinterlace-texture.c.h"
#undef gs_texture_create
#undef gs_texrender_create

static void check_case(bool gpu, int channels, enum video_format format, enum video_trc trc,
		       enum gs_color_format expected, int failed_plane, bool failed_render)
{
	obs_source_t source = {0};
	gs_texture_t *sentinel = (gs_texture_t *)&source;
	gs_texrender_t *render_sentinel = (gs_texrender_t *)&source;
	texture_calls = render_calls = 0;
	fail_plane = failed_plane;
	fail_render = failed_render;
	source.async_format = format;
	source.async_trc = trc;
	source.async_gpu_conversion = gpu;
	source.async_channel_count = channels;
	source.async_width = 1920;
	source.async_height = 1080;
	source.async_prev_texrender = render_sentinel;
	for (unsigned c = 0; c < MAX_AV_PLANES; c++) {
		source.async_prev_textures[c] = sentinel;
		source.async_convert_width[c] = 641 - c * 101;
		source.async_convert_height[c] = 479 - c * 71;
		source.async_texture_formats[c] = c % 2 ? GS_R8 : GS_R8G8;
	}
	set_deinterlace_texture_size(&source);
	unsigned count = gpu ? (unsigned)channels : 1;
	assert_int_equal(texture_calls, count);
	assert_int_equal(render_calls, gpu ? 1 : 0);
	if (gpu) {
		assert_int_equal(render_format, expected);
		assert_int_equal(render_zs, GS_ZS_NONE);
		assert_ptr_equal(source.async_prev_texrender, failed_render ? NULL : (gs_texrender_t *)&render_handle);
	} else {
		assert_ptr_equal(source.async_prev_texrender, render_sentinel);
	}
	for (unsigned c = 0; c < MAX_AV_PLANES; c++) {
		if (c >= count) {
			assert_ptr_equal(source.async_prev_textures[c], sentinel);
			continue;
		}
		assert_int_equal(allocations[c].width, gpu ? source.async_convert_width[c] : 1920);
		assert_int_equal(allocations[c].height, gpu ? source.async_convert_height[c] : 1080);
		assert_int_equal(allocations[c].format, gpu ? source.async_texture_formats[c] : expected);
		assert_int_equal(allocations[c].levels, 1);
		assert_null(allocations[c].data);
		assert_int_equal(allocations[c].flags, GS_DYNAMIC);
		assert_ptr_equal(source.async_prev_textures[c],
				 (int)c == failed_plane ? NULL : (gs_texture_t *)&handles[c]);
	}
}

static void format_selection(void **state)
{
	(void)state;
	static const struct {
		enum video_format input;
		enum gs_color_format output;
	} formats[] = {
		{VIDEO_FORMAT_RGBA, GS_RGBA},    {VIDEO_FORMAT_BGRA, GS_BGRA},    {VIDEO_FORMAT_I40A, GS_BGRA},
		{VIDEO_FORMAT_I42A, GS_BGRA},    {VIDEO_FORMAT_YUVA, GS_BGRA},    {VIDEO_FORMAT_AYUV, GS_BGRA},
		{VIDEO_FORMAT_I010, GS_RGBA16F}, {VIDEO_FORMAT_P010, GS_RGBA16F}, {VIDEO_FORMAT_I210, GS_RGBA16F},
		{VIDEO_FORMAT_I412, GS_RGBA16F}, {VIDEO_FORMAT_YA2L, GS_RGBA16F}, {VIDEO_FORMAT_P216, GS_RGBA16F},
		{VIDEO_FORMAT_P416, GS_RGBA16F}, {VIDEO_FORMAT_V210, GS_RGBA16F}, {VIDEO_FORMAT_R10L, GS_RGBA16F},
		{VIDEO_FORMAT_I420, GS_BGRX},    {VIDEO_FORMAT_NONE, GS_BGRX},
	};
	static const enum video_trc trcs[] = {VIDEO_TRC_DEFAULT, VIDEO_TRC_SRGB, VIDEO_TRC_PQ, VIDEO_TRC_HLG};
	for (unsigned f = 0; f < sizeof(formats) / sizeof(formats[0]); f++)
		for (unsigned t = 0; t < sizeof(trcs) / sizeof(trcs[0]); t++)
			for (unsigned gpu = 0; gpu < 2; gpu++)
				check_case(gpu, 3, formats[f].input, trcs[t], t >= 2 ? GS_RGBA16F : formats[f].output,
					   -1, false);
}

static void channels_and_failures(void **state)
{
	(void)state;
	for (int channels = 0; channels <= MAX_AV_PLANES; channels++)
		for (unsigned gpu = 0; gpu < 2; gpu++)
			for (int plane = -1; plane < MAX_AV_PLANES; plane++)
				for (unsigned render = 0; render < 2; render++)
					/* Characterized, not endorsed: failure overwrites the
					 * destination with NULL and does not stop later planes.
					 * Zero GPU channels still creates a texrender. */
					check_case(gpu, channels, VIDEO_FORMAT_RGBA, VIDEO_TRC_DEFAULT, GS_RGBA, plane,
						   render);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(format_selection), cmocka_unit_test(channels_and_failures)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
