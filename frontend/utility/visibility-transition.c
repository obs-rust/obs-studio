#include "visibility-transition.h"

#include <util/darray.h>
#include <util/dstr.h>

#include <string.h>

struct changes {
	const struct visibility_transition_change *change;
	obs_data_array_t *undo;
	obs_data_array_t *redo;
	size_t count;
};

static void add_entry(obs_data_array_t *entries, obs_sceneitem_t *item, obs_data_t *transition)
{
	obs_source_t *scene_source = obs_scene_get_source(obs_sceneitem_get_scene(item));
	obs_data_t *entry = obs_data_create();
	obs_data_set_string(entry, "scene", obs_source_get_uuid(scene_source));
	obs_data_set_int(entry, "id", obs_sceneitem_get_id(item));
	obs_data_set_string(entry, "name", obs_source_get_name(obs_sceneitem_get_source(item)));
	obs_data_set_obj(entry, "transition", transition);
	obs_data_array_push_back(entries, entry);
	obs_data_release(entry);
}

static void set_item_transition(obs_sceneitem_t *item, void *param)
{
	struct changes *c = param;
	const struct visibility_transition_change *change = c->change;
	obs_data_t *before = obs_sceneitem_transition_save(item, change->show);

	if (!change->id || !*change->id) {
		obs_sceneitem_set_transition(item, change->show, NULL);
		obs_sceneitem_set_transition_duration(item, change->show, 0);
	} else {
		obs_source_t *tr = obs_sceneitem_get_transition(item, change->show);

		if (!tr || strcmp(change->id, obs_source_get_id(tr)) != 0) {
			struct dstr name = {0};
			dstr_copy(&name, obs_source_get_name(obs_sceneitem_get_source(item)));
			dstr_cat(&name, " ");
			dstr_cat(&name, change->name_suffix ? change->name_suffix : "");

			tr = obs_source_create_private(change->id, name.array, NULL);
			obs_sceneitem_set_transition(item, change->show, tr);
			obs_source_release(tr);
			dstr_free(&name);

			if (obs_sceneitem_get_transition_duration(item, change->show) == 0)
				obs_sceneitem_set_transition_duration(item, change->show, change->default_duration);
		}
	}

	obs_data_t *after = obs_sceneitem_transition_save(item, change->show);
	if (strcmp(obs_data_get_json(before), obs_data_get_json(after)) != 0) {
		add_entry(c->undo, item, before);
		add_entry(c->redo, item, after);
		c->count++;
	}
	obs_data_release(before);
	obs_data_release(after);
}

struct targets {
	obs_sceneitem_t *item;
	DARRAY(obs_sceneitem_t *) items;
};

static bool collect_selected(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	struct targets *t = param;
	UNUSED_PARAMETER(scene);

	if (item != t->item && obs_sceneitem_selected(item)) {
		obs_sceneitem_addref(item);
		da_push_back(t->items, &item);
	}

	if (obs_sceneitem_is_group(item))
		obs_sceneitem_group_enum_items(item, collect_selected, t);

	return true;
}

/* Calls apply for item, then for every other selected item of scene and of
 * its groups. The items are collected first: apply creates sources, which
 * must not happen while the scene is locked for the enumeration. */
static void for_each_target(obs_scene_t *scene, obs_sceneitem_t *item, void (*apply)(obs_sceneitem_t *, void *),
			    void *param)
{
	struct targets t = {.item = item};

	da_init(t.items);
	if (scene)
		obs_scene_enum_items(scene, collect_selected, &t);

	if (item)
		apply(item, param);
	for (size_t i = 0; i < t.items.num; i++) {
		apply(t.items.array[i], param);
		obs_sceneitem_release(t.items.array[i]);
	}
	da_free(t.items);
}

static void set_entries(obs_data_t *data, obs_data_array_t *entries, bool show)
{
	if (!data)
		return;
	obs_data_set_bool(data, "show", show);
	obs_data_set_array(data, "items", entries);
}

size_t visibility_transition_set(obs_scene_t *scene, obs_sceneitem_t *item,
				 const struct visibility_transition_change *change, obs_data_t *undo, obs_data_t *redo)
{
	struct changes c = {
		.change = change,
		.undo = obs_data_array_create(),
		.redo = obs_data_array_create(),
	};

	for_each_target(scene, item, set_item_transition, &c);

	set_entries(undo, c.undo, change->show);
	set_entries(redo, c.redo, change->show);
	obs_data_array_release(c.undo);
	obs_data_array_release(c.redo);
	return c.count;
}

void visibility_transition_restore(obs_data_t *data)
{
	bool show = obs_data_get_bool(data, "show");
	obs_data_array_t *entries = obs_data_get_array(data, "items");
	size_t count = obs_data_array_count(entries);

	for (size_t i = 0; i < count; i++) {
		obs_data_t *entry = obs_data_array_item(entries, i);
		obs_source_t *source = obs_get_source_by_uuid(obs_data_get_string(entry, "scene"));
		obs_scene_t *scene = obs_group_or_scene_from_source(source);
		obs_sceneitem_t *item = obs_scene_find_sceneitem_by_id(scene, obs_data_get_int(entry, "id"));

		if (item) {
			obs_data_t *transition = obs_data_get_obj(entry, "transition");
			obs_sceneitem_transition_load(item, transition, show);
			obs_data_release(transition);
		}

		obs_source_release(source);
		obs_data_release(entry);
	}

	obs_data_array_release(entries);
}

struct duration_change {
	bool show;
	uint32_t duration;
};

static void set_item_duration(obs_sceneitem_t *item, void *param)
{
	struct duration_change *d = param;
	obs_sceneitem_set_transition_duration(item, d->show, d->duration);
}

void visibility_transition_set_duration(obs_scene_t *scene, obs_sceneitem_t *item, bool show, uint32_t duration)
{
	struct duration_change d = {show, duration};
	for_each_target(scene, item, set_item_duration, &d);
}
