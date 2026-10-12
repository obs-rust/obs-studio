#include "transform-clipboard.h"

/* Stub with the logic of OBSBasic::on_actionCopyTransform_triggered() and
 * on_actionPasteTransform_triggered(). */

void transform_clipboard_copy(obs_scene_t *scene, obs_sceneitem_t *item, struct obs_transform_info *info,
			      struct obs_sceneitem_crop *crop)
{
	UNUSED_PARAMETER(scene);
	obs_sceneitem_get_info2(item, info);
	obs_sceneitem_get_crop(item, crop);
}

struct paste_data {
	const struct obs_transform_info *info;
	const struct obs_sceneitem_crop *crop;
};

static bool paste_item(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	struct paste_data *data = param;
	UNUSED_PARAMETER(scene);

	if (!obs_sceneitem_selected(item) || obs_sceneitem_locked(item))
		return true;

	obs_sceneitem_defer_update_begin(item);
	obs_sceneitem_set_info2(item, data->info);
	obs_sceneitem_set_crop(item, data->crop);
	obs_sceneitem_defer_update_end(item);
	return true;
}

void transform_clipboard_paste(obs_scene_t *scene, const struct obs_transform_info *info,
			       const struct obs_sceneitem_crop *crop)
{
	struct paste_data data = {info, crop};
	obs_scene_enum_items(scene, paste_item, &data);
}
