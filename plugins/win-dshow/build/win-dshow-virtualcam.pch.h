/* Precompiled header (BUILD.md) for the plugins/win-dshow unity groups
 * win-dshow.virtualcam-windows. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* virtualcam.c */
#include <obs-module.h>
#include <util/platform.h>
#include "util/threading.h"
#include "plugins/win-dshow/shared-memory-queue.h"
#define COBJMACROS
#include <windows.h>
#include <mmdeviceapi.h>
#include <audioclient.h>
#include <propsys.h>
#include "plugins/win-dshow/vmic-route.h"
