/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-render-route. obs-source-render-route.c needs its
 * declarations (obs-source-render-route.h) and the enum obs_source_type,
 * enum obs_allow_direct_render, enum gs_color_space and enum gs_color_format
 * values and the OBS_SOURCE_* flags (obs.h), not the core, so the C oracle and
 * the C test can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include "obs.h"
#include "obs-source-render-route.h"
