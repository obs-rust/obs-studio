/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-size.
 * obs-source-transition-size.c reads and writes the transition fields of
 * struct obs_source (obs-internal.h), asks the clock and render rules
 * (obs-source-transition-clock.h, obs-source-transition-render.h), declares
 * its helpers in obs-source-transition-size.h, and calls the obs.h source
 * and graphics functions and calc_torquef() (graphics/math-extra.h), not
 * the rest of the core, so the C oracle can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/graphics/math-extra.h"
#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-clock.h"
#include "libobs/obs-source-transition-render.h"
#include "libobs/obs-source-transition-size.h"
