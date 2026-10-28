#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The image orientation and size check of graphics-ffmpeg.c, on plain values
 * and buffers (graphics-ffmpeg-orient.c). graphics-ffmpeg.c loads every image
 * file through FFmpeg; the orientation and the size come from the file:
 * - gs_ffmpeg_image_matrix_orientation(): display_matrix_to_orientation(), the
 *   EXIF orientation from the signs of the frame's display matrix (FFmpeg
 *   builds without the EXIF API);
 * - gs_ffmpeg_image_orient_needed(), _orient_swaps() and _orient_copy():
 *   ffmpeg_image_orient(), whether the decoded image is turned, whether its
 *   width and height swap, and the RGBA copy that turns it;
 * - gs_ffmpeg_image_too_large(): ffmpeg_image_too_large(), whether the image
 *   is refused before it is decoded.
 *
 * graphics-ffmpeg.c keeps every FFmpeg call, the orientation read, the
 * allocation (it passes the output buffer) and its free, the size update, the
 * too_large warning and flag. Nothing here allocates, takes a lock, calls into
 * libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* The EXIF orientation (1 to 8) whose display matrix has the signs of a, b,
 * c and d (matrix[0], [1], [3] and [4]), or 0 for none. */
int gs_ffmpeg_image_matrix_orientation(int32_t a, int32_t b, int32_t c, int32_t d);

/* The image is turned for this orientation (not 0 or 1). */
bool gs_ffmpeg_image_orient_needed(int orient);

/* Width and height swap for this orientation (5 to 8). */
bool gs_ffmpeg_image_orient_swaps(int orient);

/* Copies the cx x cy RGBA image src (4 bytes a pixel) into dst, turned for
 * orient; dst has room for cx * cy pixels. Only for an orientation that
 * gs_ffmpeg_image_orient_needed() says is turned. */
void gs_ffmpeg_image_orient_copy(uint8_t *dst, const uint8_t *src, size_t cx, size_t cy, int orient);

/* An image of cx x cy is larger than max_size (the device's maximum texture
 * size; 0 for unknown) in either direction. */
bool gs_ffmpeg_image_too_large(uint32_t max_size, int cx, int cy);
