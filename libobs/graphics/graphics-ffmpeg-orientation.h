#pragma once

#include <stddef.h>
#include <stdint.h>

/* Private FFmpeg accessors: borrowed data remains live during selection. */
const char *gs_image_orientation_metadata(const void *frame);
const uint8_t *gs_image_orientation_matrix(const void *frame, size_t *size);
int gs_image_orientation_convert(const int32_t *matrix);
int gs_ffmpeg_image_get_orientation(const void *frame);
