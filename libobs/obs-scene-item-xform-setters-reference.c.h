#include "libobs/build/libobs.pch.h"

/* C-reference implementation member. The original bodies remain shared with
 * the differential oracle under legacy/, as with obs-video-render-size. */
void obs_sceneitem_set_pos(obs_sceneitem_t *item, const struct vec2 *pos);
void obs_sceneitem_set_rot(obs_sceneitem_t *item, float rot);
void obs_sceneitem_set_scale(obs_sceneitem_t *item, const struct vec2 *scale);
#include "rust/obs-c-oracle/legacy/obs-scene-item-xform-setters.inc"
