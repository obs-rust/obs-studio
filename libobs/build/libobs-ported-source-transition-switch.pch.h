/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-switch.
 * obs-source-transition-switch.c reads and writes the transition fields of
 * struct obs_source (obs-internal.h), asks the transition seam
 * (obs-source-transition-seam.h) and the start rules
 * (obs-source-transition-start.h), recalculates the size and matrices
 * (obs-source-transition-size.h), and calls the obs.h source functions,
 * not the rest of the core, so the C oracle can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-seam.h"
#include "libobs/obs-source-transition-size.h"
#include "libobs/obs-source-transition-start.h"
