/* Test-only oracle: the original libobs/graphics/graphics-ffmpeg-orient.c.h,
 * unmodified, with its five functions renamed to oracle_* so they can link
 * next to the Rust shims. Nothing in it divides or allocates; the copy reads
 * and writes cx * cy * 4 bytes, so the parity test passes buffers of that
 * size. */
#include <stddef.h>
#include <stdint.h>

#define gs_ffmpeg_image_matrix_orientation oracle_gs_ffmpeg_image_matrix_orientation
#define gs_ffmpeg_image_orient_needed oracle_gs_ffmpeg_image_orient_needed
#define gs_ffmpeg_image_orient_swaps oracle_gs_ffmpeg_image_orient_swaps
#define gs_ffmpeg_image_orient_copy oracle_gs_ffmpeg_image_orient_copy
#define gs_ffmpeg_image_too_large oracle_gs_ffmpeg_image_too_large

#include "graphics/graphics-ffmpeg-orient.c.h"

#undef gs_ffmpeg_image_too_large
#undef gs_ffmpeg_image_orient_copy
#undef gs_ffmpeg_image_orient_swaps
#undef gs_ffmpeg_image_orient_needed
#undef gs_ffmpeg_image_matrix_orientation
