/* Test-only oracle: the original libobs/graphics/graphics-ffmpeg-rgba64.c.h,
 * unmodified, with its function renamed to oracle_* so it can link next to the
 * Rust shim. It calls only srgb.h's inline helpers (powf); it allocates
 * nothing and writes at most cx * cy * 4 bytes. */
#include <stddef.h>
#include <stdint.h>

#define gs_ffmpeg_image_rgba64be_rows oracle_gs_ffmpeg_image_rgba64be_rows

#include "graphics/graphics-ffmpeg-rgba64.c.h"

#undef gs_ffmpeg_image_rgba64be_rows
