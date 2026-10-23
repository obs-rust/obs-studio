/* Test-only oracle: libobs/obs-scene-extent.c.h, unmodified, with its three
 * functions renamed so they can link next to the Rust shims.
 * vec3_transform is the graphics oracle. vec2_set, vec2_sub, vec3_set and
 * obs_scene_size_from_extent stay the header inlines. */
#define obs_scene_extent_init oracle_obs_scene_extent_init
#define obs_scene_extent_add oracle_obs_scene_extent_add
#define obs_scene_extent_size oracle_obs_scene_extent_size
#define vec3_transform oracle_vec3_transform

#include "obs-scene-extent.c.h"

#undef vec3_transform
#undef obs_scene_extent_size
#undef obs_scene_extent_add
#undef obs_scene_extent_init
