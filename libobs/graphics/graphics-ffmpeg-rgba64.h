#pragma once

#include <stddef.h>
#include <stdint.h>

/* The 16-bit RGBA rows of graphics-ffmpeg.c, on plain values and buffers
 * (graphics-ffmpeg-rgba64.c). A 16-bit PNG decodes to AV_PIX_FMT_RGBA64BE;
 * ffmpeg_image_reformat_frame() turns it into 8-bit RGBA with
 * gs_ffmpeg_image_rgba64be_rows().
 *
 * graphics-ffmpeg.c keeps the format route, the allocation (it passes the
 * output buffer) and the format it stores afterwards. Nothing here allocates,
 * takes a lock, calls into libobs or keeps state.
 *
 * The alpha modes are enum gs_image_alpha_mode's values (graphics.h), which
 * graphics-ffmpeg.c checks, so this header needs no graphics header.
 *
 * Hidden in libobs: not EXPORT. */

#define GS_FFMPEG_RGBA64_ALPHA_STRAIGHT 0
#define GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB 1
#define GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY 2

/* Converts the cy rows of the big-endian 16-bit RGBA image src, src_linesize
 * bytes apart, into 8-bit RGBA in dst (cx * cy * 4 bytes), for alpha_mode:
 * straight, premultiplied in sRGB, or premultiplied. Each row converts the
 * smaller of cx and src_linesize / 8 texels, and dst advances 4 bytes per
 * texel written. Writes nothing for another alpha mode. */
void gs_ffmpeg_image_rgba64be_rows(uint8_t *dst, const uint8_t *src, int cx, int cy, size_t src_linesize,
				   int alpha_mode);
