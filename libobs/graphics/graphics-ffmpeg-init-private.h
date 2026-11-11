/* Private image API shared by the lifecycle and decode unity fragments. */
#pragma once

#include "libobs/graphics/graphics-ffmpeg-image-private.h"

bool ffmpeg_image_too_large(struct ffmpeg_image *info, int cx, int cy);
bool ffmpeg_image_init(struct ffmpeg_image *info, const char *file, uint32_t (*max_size)(void));
void ffmpeg_image_free(struct ffmpeg_image *info);
int obs_ffmpeg_image_get_buffer(struct ffmpeg_image *info, void *buffer, int flags);
int obs_ffmpeg_image_operation(struct ffmpeg_image *info, uint32_t op, int value, void *extra,
			       int (*get_buffer)(struct ffmpeg_image *, void *, int), uint32_t (*max_size)(void));
void *ffmpeg_image_reformat_frame(struct ffmpeg_image *info, AVFrame *frame, enum gs_image_alpha_mode alpha_mode);
void *gs_ffmpeg_image_decode(void *info, int alpha_mode);
