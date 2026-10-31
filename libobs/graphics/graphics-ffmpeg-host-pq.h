#pragma once

#include <stdint.h>

/* The JPEG XR PQ-to-scRGB conversion of the image loader's Windows host
 * (graphics-ffmpeg-host-windows.c), on plain buffers
 * (graphics-ffmpeg-host-pq.c). WIC decodes a PQ (HDR10) JPEG XR image to
 * GUID_WICPixelFormat32bppBGR101010; wic_image_init_internal() copies it into
 * an intermediate buffer and turns it into the GS_RGBA16F, GS_CS_709_SCRGB
 * texture data with gs_image_pq10_to_scrgb().
 *
 * graphics-ffmpeg-host-windows.c keeps every WIC call, the pixel-format test,
 * the sizes, both allocations and the stores afterwards. Nothing here
 * allocates, takes a lock, calls into libobs or keeps state, and it includes
 * no Windows header, so the C tests build it on every platform.
 *
 * Hidden in libobs: not EXPORT. */

/* Converts the src_size / 4 texels of src, each a native-endian uint32_t with
 * blue in bits 0-9, green in 10-19 and red in 20-29 (bits 30-31 unused), PQ
 * encoded with BT.2020 primaries, into dst: four native-endian half floats
 * per texel (red, green and blue in linear BT.709, 1.0 being 80 nits, and
 * alpha 1.0), 8 bytes each. dst holds dst_size bytes, which the caller
 * computes as 8 bytes per texel. */
void gs_image_pq10_to_scrgb(uint8_t *dst, uint32_t dst_size, const uint8_t *src, uint32_t src_size);
