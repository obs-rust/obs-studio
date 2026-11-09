/* Precompiled header (BUILD.md): version-sensitive FFmpeg resampler adapter. */
#pragma once
#include "libobs/media-io/audio-resampler.h"
#include "libobs/media-io/audio-resampler-drift.h"
#include "libobs/media-io/audio-resampler-kernel.h"
#include "libobs/util/base.h"
#include <libavutil/avutil.h>
#include <libswresample/swresample.h>
#include <inttypes.h>
