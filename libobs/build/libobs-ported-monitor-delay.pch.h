/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-monitor-delay: the WASAPI monitor's delay decisions, speaker
 * layout and video stall tracker, without windows.h, so the C oracle and the
 * C tests compile them on their own. C. Its implementation files include
 * this header and nothing else. */

#pragma once

#include "audio-monitoring/monitor-delay.h"
#include "audio-monitoring/monitor-speaker-layout.h"
#include "audio-monitoring/monitor-video-sync.h"

#include <stdlib.h>
