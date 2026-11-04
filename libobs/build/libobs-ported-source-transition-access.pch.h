/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-access.
 * obs-source-transition-access.c reads and writes the transition fields of
 * struct obs_source (obs-internal.h), asks the transition seam
 * (obs-source-transition-seam.h) and the start rules
 * (obs-source-transition-start.h), and calls the obs.h source and
 * transition functions, not the rest of the core, so the C oracle can
 * compile it on its own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-seam.h"
#include "libobs/obs-source-transition-start.h"
