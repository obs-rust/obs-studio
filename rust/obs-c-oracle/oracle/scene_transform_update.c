/* Test-only oracle: the original libobs/obs-scene-transform-update.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stdbool.h>
#include <stdint.h>

#define obs_scene_transform_update_scene_size oracle_obs_scene_transform_update_scene_size
#define obs_scene_transform_update_item oracle_obs_scene_transform_update_item

#include "obs-scene-transform-update.c.h"

#undef obs_scene_transform_update_item
#undef obs_scene_transform_update_scene_size
