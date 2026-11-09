/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition.
 * obs-source-transition.c reads and writes the transition fields of
 * struct obs_source and its children's audio fields (obs-internal.h), asks
 * the transition checks (obs-source-transition-seam.h), the render and
 * clock rules (obs-source-transition-render.h,
 * obs-source-transition-sample-time.h, obs-source-transition-clock.h) and
 * the stop helpers (obs-source-transition-stop.h), copies its state into
 * struct transition_state (obs-source-transition-state.h), and calls the
 * obs.h source and graphics functions, not the rest of the core, so the C
 * oracle can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-clock.h"
#include "libobs/obs-source-transition-render.h"
#include "libobs/obs-source-transition-sample-time.h"
#include "libobs/obs-source-transition-seam.h"
#include "libobs/obs-source-transition-state.h"
#include "libobs/obs-source-transition-stop.h"
