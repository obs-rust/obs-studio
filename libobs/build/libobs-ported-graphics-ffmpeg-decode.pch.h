/* Private C reference for the extracted still-image decoder. */
#pragma once

#include "graphics/graphics.h"
#include "util/base.h"
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include "libobs/graphics/graphics-ffmpeg-image-private.h"

void *gs_ffmpeg_image_decode(void *info, int alpha_mode);
bool ffmpeg_image_too_large(struct ffmpeg_image *info, int width, int height);
void *ffmpeg_image_reformat_frame(struct ffmpeg_image *info, AVFrame *frame, enum gs_image_alpha_mode alpha_mode);

static void *ffmpeg_image_decode(struct ffmpeg_image *info, enum gs_image_alpha_mode alpha_mode);
