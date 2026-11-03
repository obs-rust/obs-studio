/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-slots.
 * obs-source-transition-slots.c reads and writes the transition fields of
 * struct obs_source (obs-internal.h), asks the transition seam
 * (obs-source-transition-seam.h) and the start and load rules
 * (obs-source-transition-start.h, obs-source-transition-load.h), and calls
 * the obs.h source functions, not the rest of the core, so the C oracle can
 * compile it on its own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-load.h"
#include "libobs/obs-source-transition-seam.h"
#include "libobs/obs-source-transition-start.h"
