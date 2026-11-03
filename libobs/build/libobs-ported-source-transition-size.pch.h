/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-size.
 * obs-source-transition-size.c and obs-source-transition-restore.c read and
 * write the transition fields of struct obs_source (obs-internal.h), ask the
 * clock, render, load, start and seam rules (obs-source-transition-clock.h,
 * obs-source-transition-render.h, obs-source-transition-load.h,
 * obs-source-transition-start.h, obs-source-transition-seam.h), declare the
 * sizing helpers in obs-source-transition-size.h, and call the obs.h source,
 * data and graphics functions and calc_torquef() (graphics/math-extra.h),
 * not the rest of the core, so the C oracle can compile them on their own.
 * The implementation files include this header and nothing else. */

#pragma once

#include "libobs/graphics/math-extra.h"
#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-clock.h"
#include "libobs/obs-source-transition-load.h"
#include "libobs/obs-source-transition-render.h"
#include "libobs/obs-source-transition-seam.h"
#include "libobs/obs-source-transition-size.h"
#include "libobs/obs-source-transition-start.h"
