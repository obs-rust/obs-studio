/* Precompiled header (BUILD.md) for the libobs unity groups libobs.pulse. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* audio-monitoring/pulse/pulseaudio-enum-devices.c */
#include <obs-internal.h>
#include "audio-monitoring/pulse/pulseaudio-wrapper.h"

/* audio-monitoring/pulse/pulseaudio-output.c */
#include "obs-internal.h"
#include "util/util_uint64.h"
#include "audio-monitoring/pulse/pulseaudio-stream-flags.h"

#include <pulse/timeval.h>

/* audio-monitoring/pulse/pulseaudio-wrapper.c */
#include <pthread.h>

#include <pulse/thread-mainloop.h>

#include <util/base.h>
#include <obs.h>
