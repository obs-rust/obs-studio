/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-scene-item-transform. obs-scene-item-transform.c needs its
 * declarations (obs-scene-item-transform.h), struct obs_sceneitem_crop and
 * OBS_BOUNDS_NONE (obs.h), the matrix4 and vec2 helpers and RAD()
 * (graphics/), obs_scene_cropped_size() and obs_scene_scale_crop()
 * (obs-scene-crop.h) and memset(), not the core, so the C oracle and the C
 * test can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include <string.h>

#include "obs.h"
#include "graphics/math-defs.h"
#include "graphics/matrix4.h"
#include "graphics/vec2.h"
#include "obs-scene-crop.h"
#include "obs-scene-item-transform.h"
