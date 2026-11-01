#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The pixel route of graphics-ffmpeg.c, on plain values and buffers
 * (graphics-ffmpeg-route.c). ffmpeg_image_reformat_frame() turns a decoded
 * frame into the texture data:
 * - gs_ffmpeg_image_plan(): which way the frame goes (a straight copy, the
 *   premultiplied rows, the 16-bit rows or swscale), which premultiply runs,
 *   what swscale is given and the pixel format afterwards;
 * - gs_ffmpeg_image_color_format(): convert_format(), the texture format of
 *   that pixel format;
 * - gs_ffmpeg_image_copy_rows(): the copy of ffmpeg_image_copy_data_straight().
 *
 * graphics-ffmpeg.c keeps every FFmpeg and swscale call and their logs, every
 * allocation (it passes the output buffer), the premultiply loops, the
 * swscale output copy and the format stores. Nothing here allocates, takes a
 * lock, calls into libobs or keeps state.
 *
 * The pixel formats are enum AVPixelFormat's values and the texture formats
 * enum gs_color_format's (graphics.h), which graphics-ffmpeg.c checks, so this
 * header needs no FFmpeg or graphics header.
 *
 * Hidden in libobs: not EXPORT. */

#define GS_FFMPEG_IMAGE_PIX_FMT_YUV420P 0
#define GS_FFMPEG_IMAGE_PIX_FMT_YUV422P 4
#define GS_FFMPEG_IMAGE_PIX_FMT_YUV444P 5
#define GS_FFMPEG_IMAGE_PIX_FMT_YUV411P 7
#define GS_FFMPEG_IMAGE_PIX_FMT_YUVJ420P 12
#define GS_FFMPEG_IMAGE_PIX_FMT_YUVJ422P 13
#define GS_FFMPEG_IMAGE_PIX_FMT_YUVJ444P 14
#define GS_FFMPEG_IMAGE_PIX_FMT_RGBA 26
#define GS_FFMPEG_IMAGE_PIX_FMT_BGRA 28
#define GS_FFMPEG_IMAGE_PIX_FMT_YUV440P 31
#define GS_FFMPEG_IMAGE_PIX_FMT_YUVJ440P 32
#define GS_FFMPEG_IMAGE_PIX_FMT_RGBA64BE 104
#define GS_FFMPEG_IMAGE_PIX_FMT_BGR0 121
#define GS_FFMPEG_IMAGE_PIX_FMT_YUVJ411P 138

#define GS_FFMPEG_IMAGE_GS_RGBA 3
#define GS_FFMPEG_IMAGE_GS_BGRX 4
#define GS_FFMPEG_IMAGE_GS_BGRA 5
#define GS_FFMPEG_IMAGE_GS_RGBA16 7

/* The alpha modes are enum gs_image_alpha_mode's values. */
#define GS_FFMPEG_IMAGE_ALPHA_STRAIGHT 0
#define GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY_SRGB 1
#define GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY 2

/* The frame is copied as it is (gs_ffmpeg_image_copy_rows()). */
#define GS_FFMPEG_IMAGE_ROUTE_COPY 0
/* The frame's rows are premultiplied into the output (8-bit RGBA or BGRA). */
#define GS_FFMPEG_IMAGE_ROUTE_PREMULTIPLY_ROWS 1
/* The 16-bit rows (gs_ffmpeg_image_rgba64be_rows()). */
#define GS_FFMPEG_IMAGE_ROUTE_RGBA64 2
/* swscale to BGRA, then premultiplied over the whole output. */
#define GS_FFMPEG_IMAGE_ROUTE_SWS 3

#define GS_FFMPEG_IMAGE_PREMULTIPLY_NONE 0
#define GS_FFMPEG_IMAGE_PREMULTIPLY_SRGB 1
#define GS_FFMPEG_IMAGE_PREMULTIPLY_LINEAR 2

struct gs_ffmpeg_image_plan {
	/* GS_FFMPEG_IMAGE_ROUTE_* */
	int route;
	/* GS_FFMPEG_IMAGE_PREMULTIPLY_*: what the premultiplied rows and the
	 * swscale route run. */
	int premultiply;
	/* The pixel format swscale is given, and whether it is full range. */
	int sws_format;
	bool full_range;
	/* The pixel format of the output. */
	int out_format;
};

/* The route of a frame of pixel format pix_fmt, loaded for alpha_mode. */
struct gs_ffmpeg_image_plan gs_ffmpeg_image_plan(int pix_fmt, int alpha_mode);

/* The texture format (GS_FFMPEG_IMAGE_GS_*) of the output pixel format
 * pix_fmt. */
int gs_ffmpeg_image_color_format(int pix_fmt);

/* Copies the cy rows of the RGBA image src, src_linesize bytes apart, into
 * dst (cx * cy * 4 bytes): one copy when src_linesize is cx * 4, otherwise
 * the smaller of the two line sizes from each row. */
void gs_ffmpeg_image_copy_rows(uint8_t *dst, const uint8_t *src, size_t cx, size_t cy, size_t src_linesize);
