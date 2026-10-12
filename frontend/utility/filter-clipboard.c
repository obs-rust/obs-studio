#include "filter-clipboard.h"

#include <util/darray.h>

struct paste_targets {
	obs_source_t *src;
	DARRAY(obs_source_t *) sources;
};

static bool has_source(struct paste_targets *t, obs_source_t *source)
{
	for (size_t i = 0; i < t->sources.num; i++) {
		if (t->sources.array[i] == source)
			return true;
	}
	return false;
}

static bool collect_selected(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	struct paste_targets *t = param;
	obs_source_t *source = obs_sceneitem_get_source(item);
	UNUSED_PARAMETER(scene);

	if (obs_sceneitem_selected(item) && source != t->src && !has_source(t, source)) {
		source = obs_source_get_ref(source);
		if (source)
			da_push_back(t->sources, &source);
	}

	if (obs_sceneitem_is_group(item))
		obs_sceneitem_group_enum_items(item, collect_selected, t);

	return true;
}

static void add_entry(obs_data_array_t *entries, obs_source_t *source, obs_data_array_t *filters)
{
	obs_data_t *entry = obs_data_create();
	obs_data_set_string(entry, "uuid", obs_source_get_uuid(source));
	obs_data_set_array(entry, "array", filters);
	obs_data_array_push_back(entries, entry);
	obs_data_release(entry);
}

size_t filter_clipboard_paste(obs_scene_t *scene, obs_source_t *src, obs_data_t *undo, obs_data_t *redo)
{
	struct paste_targets t = {.src = src};
	obs_data_array_t *undo_entries;
	obs_data_array_t *redo_entries;
	size_t count;

	if (!scene || !src)
		return 0;

	da_init(t.sources);
	obs_scene_enum_items(scene, collect_selected, &t);

	undo_entries = obs_data_array_create();
	redo_entries = obs_data_array_create();

	for (size_t i = 0; i < t.sources.num; i++) {
		obs_source_t *dst = t.sources.array[i];
		obs_data_array_t *before = obs_source_backup_filters(dst);
		obs_source_copy_filters(dst, src);
		obs_data_array_t *after = obs_source_backup_filters(dst);

		add_entry(undo_entries, dst, before);
		add_entry(redo_entries, dst, after);

		obs_data_array_release(before);
		obs_data_array_release(after);
	}

	if (undo)
		obs_data_set_array(undo, "sources", undo_entries);
	if (redo)
		obs_data_set_array(redo, "sources", redo_entries);

	obs_data_array_release(undo_entries);
	obs_data_array_release(redo_entries);

	count = t.sources.num;
	for (size_t i = 0; i < t.sources.num; i++)
		obs_source_release(t.sources.array[i]);
	da_free(t.sources);
	return count;
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
