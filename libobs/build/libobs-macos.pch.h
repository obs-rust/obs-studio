/* Precompiled header (BUILD.md) for the libobs unity groups libobs.macos. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* audio-monitoring/osx/coreaudio-enum-devices.c */
#include <CoreFoundation/CFString.h>
#include <CoreAudio/CoreAudio.h>

#include "obs-internal.h"
#include "util/dstr.h"
#include "util/apple/cfstring-utils.h"

#include "audio-monitoring/osx/mac-helpers.h"

/* audio-monitoring/osx/coreaudio-output.c */
#include <AudioUnit/AudioUnit.h>
#include <AudioToolbox/AudioQueue.h>

#include "audio-monitoring/osx/coreaudio-pacing.h"
#include "media-io/audio-resampler.h"
#include "util/deque.h"
#include "util/threading.h"
#include "util/platform.h"
#include "util/darray.h"
