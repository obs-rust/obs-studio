/* Test-only oracle: libobs/obs-scene-crop.c.h, unmodified, with its four
 * functions renamed so they can link next to the Rust shims. */
#define obs_scene_cropped_size oracle_obs_scene_cropped_size
#define obs_scene_scale_crop oracle_obs_scene_scale_crop
#define obs_scene_crop_offset oracle_obs_scene_crop_offset
#define obs_scene_clamp_crop oracle_obs_scene_clamp_crop

#include "obs-scene-crop.c.h"

#undef obs_scene_clamp_crop
#undef obs_scene_crop_offset
#undef obs_scene_scale_crop
#undef obs_scene_cropped_size
