/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-deinterlace.
 * obs-source-deinterlace.c, the deinterlace of an async source, reads and
 * changes the async and deinterlace fields of struct obs_source and the
 * core's video (obs-internal.h), asks the selection, pacing and draw
 * decisions (obs-source-deinterlace-select.h, -pacing.h, -draw.h) and the
 * technique names (obs-source-draw-technique.h), and calls the frame cache,
 * the async texture upload and the obs.h graphics functions, not the rest
 * of the core, so the C oracle can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-deinterlace-draw.h"
#include "libobs/obs-source-deinterlace-pacing.h"
#include "libobs/obs-source-deinterlace-select.h"
#include "libobs/obs-source-draw-technique.h"
#include "libobs/obs-source-frame-copy.h"
