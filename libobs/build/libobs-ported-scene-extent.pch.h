/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-scene-extent. obs-scene-extent.c.h needs its declarations
 * (obs-scene-extent.h), vec3_transform, M_INFINITE and
 * obs_scene_size_from_extent(), all without the core, so a C test and the
 * oracle can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include "libobs/graphics/math-defs.h"
#include "libobs/graphics/vec3.h"
#include "libobs/obs-scene-extent.h"
