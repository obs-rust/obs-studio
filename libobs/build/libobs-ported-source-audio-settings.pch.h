/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-audio-settings.
 * obs-source-audio-settings.c reads and writes the audio settings fields of
 * struct obs_source (obs-internal.h) and asks the monitoring rules
 * (obs-source-monitoring.h), not the rest of the core, so the C oracle can
 * compile it on its own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-monitoring.h"
