/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-display.
 * obs-display.c, the display, reads and changes struct obs_display and the
 * core's display list (obs-internal.h), asks the clear rules
 * (obs-canvas-draw.h) and the host's two clear answers
 * (obs-host/display-clear-workaround.h, obs-host/display-srgb-clear.h), and
 * calls the obs.h graphics functions, not the rest of the core, so the C
 * oracle can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-canvas-draw.h"
#include "libobs/obs-host/display-clear-workaround.h"
#include "libobs/obs-host/display-srgb-clear.h"
