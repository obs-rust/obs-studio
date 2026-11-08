/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-stop.
 * obs-source-transition-stop.c reads and writes the transition fields of
 * struct obs_source (obs-internal.h), asks the clock rule
 * (obs-source-transition-clock.h), defines the helpers its private header
 * declares (obs-source-transition-stop.h), and calls the obs.h source
 * functions, not the rest of the core, so the C oracle can compile it on
 * its own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-transition-clock.h"
#include "libobs/obs-source-transition-stop.h"
