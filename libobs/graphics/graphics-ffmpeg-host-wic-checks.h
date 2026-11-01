#pragma once

#include <stdbool.h>
#include <stddef.h>

/* Two checks of the image loader's Windows host
 * (graphics-ffmpeg-host-windows.c), on plain values
 * (graphics-ffmpeg-host-wic-checks.c): the file-name check at the top of
 * gs_image_host_fallback_load() and the pixel-format pick of
 * wic_image_init_internal().
 *
 * graphics-ffmpeg-host-windows.c keeps the strlen(), both pixel-format GUID
 * compares (their results are passed in), every WIC call, the sizes, both
 * allocations, the stores and every log line. Nothing here allocates, takes
 * a lock or keeps state, and it includes no Windows header, so the C tests
 * build it on every platform.
 *
 * Hidden in libobs: not EXPORT. */

/* What wic_image_init_internal() does with a decoded frame. */
enum gs_image_wic_kind {
	/* Any other pixel format: logged, and the image fails. */
	GS_IMAGE_WIC_UNSUPPORTED,
	/* GUID_WICPixelFormat64bppRGBAHalf: copied as it is. */
	GS_IMAGE_WIC_COPY,
	/* GUID_WICPixelFormat32bppBGR101010: through gs_image_pq10_to_scrgb(). */
	GS_IMAGE_WIC_PQ10,
};

/* Whether gs_image_host_fallback_load() refuses file, whose strlen() is len,
 * before it tries WIC. */
bool gs_image_host_fallback_refuses(const char *file, size_t len);

/* The kind for a frame whose pixel format is GUID_WICPixelFormat64bppRGBAHalf
 * (scrgb) or GUID_WICPixelFormat32bppBGR101010 (pq10). */
enum gs_image_wic_kind gs_image_wic_kind(bool scrgb, bool pq10);
