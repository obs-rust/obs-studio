/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.monitor-wasapi: the WASAPI monitor's speaker layout and video stall
 * tracker, without windows.h, so the C oracle and the C tests compile them on
 * their own. C. Its implementation files include this header and nothing
 * else. */

#pragma once

#include "audio-monitoring/monitor-speaker-layout.h"
#include "audio-monitoring/monitor-video-sync.h"
