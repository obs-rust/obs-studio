#include "filter-clipboard.h"

/* Stub with the logic of OBSBasic::on_actionPasteFilters_triggered(), which
 * pastes into the current item of the source list only: here, the first
 * selected item of the scene. */

struct first_selected {
	obs_sceneitem_t *item;
};

static bool find_first_selected(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	struct first_selected *first = param;
	UNUSED_PARAMETER(scene);

	if (!obs_sceneitem_selected(item))
		return true;

	first->item = item;
	return false;
}

static void add_entry(obs_data_t *data, obs_source_t *source, obs_data_array_t *filters)
{
	obs_data_array_t *entries = obs_data_array_create();
	obs_data_t *entry = obs_data_create();
	obs_data_set_string(entry, "uuid", obs_source_get_uuid(source));
	obs_data_set_array(entry, "array", filters);
	obs_data_array_push_back(entries, entry);
	obs_data_set_array(data, "sources", entries);
	obs_data_release(entry);
	obs_data_array_release(entries);
}

size_t filter_clipboard_paste(obs_scene_t *scene, obs_source_t *src, obs_data_t *undo, obs_data_t *redo)
{
	struct first_selected first = {0};
	obs_source_t *dst;

	if (!scene || !src)
		return 0;

	obs_scene_enum_items(scene, find_first_selected, &first);
	dst = obs_sceneitem_get_source(first.item);
	if (!dst || dst == src)
		return 0;

	obs_data_array_t *before = obs_source_backup_filters(dst);
	obs_source_copy_filters(dst, src);
	obs_data_array_t *after = obs_source_backup_filters(dst);

	if (undo)
		add_entry(undo, dst, before);
	if (redo)
		add_entry(redo, dst, after);

	obs_data_array_release(before);
	obs_data_array_release(after);
	return 1;
}

void filter_clipboard_restore(obs_data_t *data, void (*restored)(obs_source_t *source, void *param), void *param)
{
	obs_data_array_t *entries = obs_data_get_array(data, "sources");
	size_t count = obs_data_array_count(entries);

	for (size_t i = 0; i < count; i++) {
		obs_data_t *entry = obs_data_array_item(entries, i);
		obs_data_array_t *filters = obs_data_get_array(entry, "array");
		obs_source_t *source = obs_get_source_by_uuid(obs_data_get_string(entry, "uuid"));

		if (source) {
			obs_source_restore_filters(source, filters);
			if (restored)
				restored(source, param);
			obs_source_release(source);
		}

		obs_data_array_release(filters);
		obs_data_release(entry);
	}

	obs_data_array_release(entries);
}
