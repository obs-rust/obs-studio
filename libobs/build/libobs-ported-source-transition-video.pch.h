/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-video.
 * obs-source-transition-video.c reads the transition fields of struct
 * obs_source (obs-internal.h), asks the render rules
 * (obs-source-transition-render.h) and the colour-space mix
 * (obs-source-transition-clock.h), calls the clock read and stop helpers
 * (obs-source-transition-stop.h), the size helpers
 * (obs-source-transition-size.h) and transition_valid()
 * (obs-source-transition-seam.h), and the obs.h source and graphics
 * functions, not the rest of the core, so the C oracle can compile it on
 * its own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-clock.h"
#include "libobs/obs-source-transition-render.h"
#include "libobs/obs-source-transition-seam.h"
#include "libobs/obs-source-transition-size.h"
#include "libobs/obs-source-transition-state.h"
#include "libobs/obs-source-transition-stop.h"
