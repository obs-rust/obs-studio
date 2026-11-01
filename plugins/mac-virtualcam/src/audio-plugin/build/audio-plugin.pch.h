/* Precompiled header (BUILD.md) for the plugins/mac-virtualcam/src/audio-plugin unity groups
 * audio-plugin.macos. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* OBSVirtualMicrophone.c */
#include <CoreAudio/AudioServerPlugIn.h>
#include <fcntl.h>
#include <mach/mach_time.h>
#include <pthread.h>
#include <stdbool.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

#include "vmic-clock.h"
#include "vmic-shm.h"
