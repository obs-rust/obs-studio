/* Private C reference for the extracted still-image decoder. */
#pragma once

#include "graphics/graphics.h"
#include "util/base.h"
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include "libobs/graphics/graphics-ffmpeg-init-private.h"

static void *ffmpeg_image_decode(struct ffmpeg_image *info, enum gs_image_alpha_mode alpha_mode);
