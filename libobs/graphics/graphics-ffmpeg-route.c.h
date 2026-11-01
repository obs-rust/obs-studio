#include "libobs/build/libobs-ported-graphics-ffmpeg-route.pch.h"

/* The JPEG decoders output the deprecated full-range YUVJ formats. swscale
 * still takes them, but logs "deprecated pixel format used" for every
 * context made from one, so each decoded JPEG logged it once (#2334). Give
 * swscale the matching YUV format and say the range is full instead; the
 * pixels come out the same. Other formats are passed as they are. */
static int ffmpeg_image_sws_format(int format, bool *full_range)
{
	*full_range = true;
	switch (format) {
	case GS_FFMPEG_IMAGE_PIX_FMT_YUVJ420P:
		return GS_FFMPEG_IMAGE_PIX_FMT_YUV420P;
	case GS_FFMPEG_IMAGE_PIX_FMT_YUVJ422P:
		return GS_FFMPEG_IMAGE_PIX_FMT_YUV422P;
	case GS_FFMPEG_IMAGE_PIX_FMT_YUVJ444P:
		return GS_FFMPEG_IMAGE_PIX_FMT_YUV444P;
	case GS_FFMPEG_IMAGE_PIX_FMT_YUVJ440P:
		return GS_FFMPEG_IMAGE_PIX_FMT_YUV440P;
	case GS_FFMPEG_IMAGE_PIX_FMT_YUVJ411P:
		return GS_FFMPEG_IMAGE_PIX_FMT_YUV411P;
	default:
		*full_range = false;
		return format;
	}
}

static int ffmpeg_image_premultiply(int alpha_mode)
{
	if (alpha_mode == GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY_SRGB)
		return GS_FFMPEG_IMAGE_PREMULTIPLY_SRGB;
	else if (alpha_mode == GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY)
		return GS_FFMPEG_IMAGE_PREMULTIPLY_LINEAR;
	return GS_FFMPEG_IMAGE_PREMULTIPLY_NONE;
}

struct gs_ffmpeg_image_plan gs_ffmpeg_image_plan(int pix_fmt, int alpha_mode)
{
	struct gs_ffmpeg_image_plan plan = {0};

	plan.sws_format = ffmpeg_image_sws_format(pix_fmt, &plan.full_range);
	plan.out_format = pix_fmt;

	if (pix_fmt == GS_FFMPEG_IMAGE_PIX_FMT_BGR0) {
		plan.route = GS_FFMPEG_IMAGE_ROUTE_COPY;
	} else if (pix_fmt == GS_FFMPEG_IMAGE_PIX_FMT_RGBA || pix_fmt == GS_FFMPEG_IMAGE_PIX_FMT_BGRA) {
		if (alpha_mode == GS_FFMPEG_IMAGE_ALPHA_STRAIGHT) {
			plan.route = GS_FFMPEG_IMAGE_ROUTE_COPY;
		} else {
			plan.route = GS_FFMPEG_IMAGE_ROUTE_PREMULTIPLY_ROWS;
			plan.premultiply = ffmpeg_image_premultiply(alpha_mode);
		}
	} else if (pix_fmt == GS_FFMPEG_IMAGE_PIX_FMT_RGBA64BE) {
		plan.route = GS_FFMPEG_IMAGE_ROUTE_RGBA64;
		plan.out_format = GS_FFMPEG_IMAGE_PIX_FMT_RGBA;
	} else {
		plan.route = GS_FFMPEG_IMAGE_ROUTE_SWS;
		plan.premultiply = ffmpeg_image_premultiply(alpha_mode);
		plan.out_format = GS_FFMPEG_IMAGE_PIX_FMT_BGRA;
	}

	return plan;
}

int gs_ffmpeg_image_color_format(int pix_fmt)
{
	switch (pix_fmt) {
	case GS_FFMPEG_IMAGE_PIX_FMT_RGBA:
		return GS_FFMPEG_IMAGE_GS_RGBA;
	case GS_FFMPEG_IMAGE_PIX_FMT_BGRA:
		return GS_FFMPEG_IMAGE_GS_BGRA;
	case GS_FFMPEG_IMAGE_PIX_FMT_BGR0:
		return GS_FFMPEG_IMAGE_GS_BGRX;
	case GS_FFMPEG_IMAGE_PIX_FMT_RGBA64BE:
		return GS_FFMPEG_IMAGE_GS_RGBA16;
	default:
		return GS_FFMPEG_IMAGE_GS_BGRX;
	}
}

void gs_ffmpeg_image_copy_rows(uint8_t *dst, const uint8_t *src, size_t cx, size_t cy, size_t src_linesize)
{
	const size_t linesize = cx * 4;
	const size_t totalsize = cy * linesize;

	if (linesize != src_linesize) {
		const size_t min_line = linesize < src_linesize ? linesize : src_linesize;

		for (size_t y = 0; y < cy; y++) {
			memcpy(dst, src, min_line);
			dst += linesize;
			src += src_linesize;
		}
	} else {
		memcpy(dst, src, totalsize);
	}
}
