/* The metadata fixtures preserve the real built-in IDs characterized in libobs. */
#define group_info oracle_scene_source_type_group_info
#define scene_info oracle_scene_source_type_scene_info
#define obs_source_type_is_group oracle_obs_source_type_is_group
#define obs_source_type_is_scene oracle_obs_source_type_is_scene
#include "libobs/obs-scene-source-type-queries.c.h"
#undef obs_source_type_is_group
#undef obs_source_type_is_scene

const struct obs_source_info group_info = {.id = "group"};
const struct obs_source_info scene_info = {.id = "scene"};
#undef group_info
#undef scene_info
