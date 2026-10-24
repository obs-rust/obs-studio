/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-frame-copy. obs-source-frame-copy.c needs its
 * declarations (obs-source-frame-copy.h), the video_format and
 * video_range_type enums and the format_is_yuv() and resolve_video_range()
 * inlines (media-io/video-io.h) and memset, so the C oracle and the C test
 * can compile it on their own. The implementation file includes this header
 * and nothing else. */

#pragma once

#include <string.h>

#include "media-io/video-io.h"
#include "obs-source-frame-copy.h"
