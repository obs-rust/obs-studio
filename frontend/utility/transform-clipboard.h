#pragma once

#include <obs.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Copy Transform and Paste Transform (Edit > Transform), without Qt.
 *
 * transform_clipboard_copy() reads the transform and crop of item, which is
 * in scene or inside one of its groups. The transform is given in canvas
 * coordinates: for an item inside a group, the group's position, scale and
 * rotation are folded in, as ungrouping the item would. So the copy says
 * where the item is on the canvas, wherever it is pasted.
 *
 * transform_clipboard_paste() applies a copied transform and crop to every
 * selected, unlocked item of scene, including the items inside its groups.
 * A group is pasted before its items. For an item inside a group, the
 * transform is converted into the group's coordinates, so the item lands
 * where the copied item was on the canvas. */
void transform_clipboard_copy(obs_scene_t *scene, obs_sceneitem_t *item, struct obs_transform_info *info,
			      struct obs_sceneitem_crop *crop);
void transform_clipboard_paste(obs_scene_t *scene, const struct obs_transform_info *info,
			       const struct obs_sceneitem_crop *crop);

#ifdef __cplusplus
}
#endif
