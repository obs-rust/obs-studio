/* Precompiled header (BUILD.md) for the libobs unity groups libobs.media. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* graphics/graphics-ffmpeg.c */
#include "graphics/graphics.h"
#include "graphics/graphics-internal.h"

#include "graphics/half.h"
#include "graphics/srgb.h"
#include <obs-ffmpeg-compat.h>
#include <util/dstr.h>
#include <util/platform.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/imgutils.h>
#include <libswscale/swscale.h>

#ifdef _WIN32
#include <wincodec.h>
#pragma comment(lib, "windowscodecs.lib")
#endif

/* graphics/image-file.c */
#include "graphics/image-file.h"
#include "util/base.h"
#include "util/platform.h"
#include "util/dstr.h"
#include "graphics/vec4.h"
#include "graphics/libnsgif/nsgif.h"

/* media-io/audio-resampler-ffmpeg.c */
#include "util/bmem.h"
#include "media-io/audio-resampler.h"
#include "media-io/audio-io.h"
#include <libavutil/avutil.h>
#include <libswresample/swresample.h>
#include <inttypes.h>
#include <math.h>

/* media-io/media-remux.c */
#include "media-io/media-remux.h"

#include <libavcodec/version.h>
#include <sys/types.h>
#include <sys/stat.h>

/* media-io/video-scaler-ffmpeg.c */
#include "media-io/video-scaler.h"

#include <libavutil/opt.h>

/* graphics/graphics-ffmpeg.c: FFmpeg 8.1's EXIF API (see the file) */
#define FFMPEG_HAS_EXIF_API (LIBAVCODEC_VERSION_INT >= AV_VERSION_INT(62, 13, 100))
#if FFMPEG_HAS_EXIF_API
#include <libavcodec/exif.h>
#endif
