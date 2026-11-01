/* Test-only oracle: the original libobs/graphics/graphics-ffmpeg-route.c.h,
 * unmodified, with its functions renamed to oracle_* so it can link next to the
 * Rust shims. The plan's rename is function-like, so struct gs_ffmpeg_image_plan
 * keeps its tag. It calls only memcpy; it allocates nothing and writes at most
 * cx * cy * 4 bytes. */
#include <stddef.h>
#include <stdint.h>

#define gs_ffmpeg_image_plan(pix_fmt, alpha_mode) oracle_gs_ffmpeg_image_plan(pix_fmt, alpha_mode)
#define gs_ffmpeg_image_color_format oracle_gs_ffmpeg_image_color_format
#define gs_ffmpeg_image_copy_rows oracle_gs_ffmpeg_image_copy_rows

#include "graphics/graphics-ffmpeg-route.c.h"

#undef gs_ffmpeg_image_plan
#undef gs_ffmpeg_image_color_format
#undef gs_ffmpeg_image_copy_rows

/* The layout of struct gs_ffmpeg_image_plan: size, alignment, then each
 * field's offset in declaration order. Returns the count written. */
size_t oracle_image_route_plan_layout(size_t *out);
size_t oracle_image_route_plan_layout(size_t *out)
{
	size_t n = 0;
	out[n++] = sizeof(struct gs_ffmpeg_image_plan);
	out[n++] = _Alignof(struct gs_ffmpeg_image_plan);
	out[n++] = offsetof(struct gs_ffmpeg_image_plan, route);
	out[n++] = offsetof(struct gs_ffmpeg_image_plan, premultiply);
	out[n++] = offsetof(struct gs_ffmpeg_image_plan, sws_format);
	out[n++] = offsetof(struct gs_ffmpeg_image_plan, full_range);
	out[n++] = offsetof(struct gs_ffmpeg_image_plan, out_format);
	return n;
}
