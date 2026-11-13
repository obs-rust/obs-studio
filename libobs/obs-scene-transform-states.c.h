#include "libobs/build/libobs.pch.h"

/* This translation unit keeps the existing bounds normalizer local, just
 * as obs-scene-item-bounds-setters.c.h does in the core-source group. */
/* A bounds type outside the enum (a hand-edited or foreign scene
 * collection, undo data, a plugin) has no bounding box: OBS_BOUNDS_NONE.
 * Takes the int64 a load reads, before any cast truncates it, and an enum
 * value from a setter (an unsigned or a signed int, by compiler). */
static inline enum obs_bounds_type valid_bounds_type(int64_t type)
{
	if (type < OBS_BOUNDS_NONE || type > OBS_BOUNDS_MAX_ONLY)
		return OBS_BOUNDS_NONE;
	return (enum obs_bounds_type)type;
}

struct passthrough {
	obs_data_array_t *ids;
	obs_data_array_t *scenes_and_groups;
	bool all_items;
};

bool save_transform_states(obs_scene_t *scene, obs_sceneitem_t *item, void *vp_pass);
bool save_transform_states(obs_scene_t *scene, obs_sceneitem_t *item, void *vp_pass)
{
	struct passthrough *pass = (struct passthrough *)vp_pass;
	if (obs_sceneitem_selected(item) || pass->all_items) {
		obs_data_t *temp = obs_data_create();
		obs_data_array_t *item_ids = (obs_data_array_t *)pass->ids;

		struct obs_transform_info info;
		struct obs_sceneitem_crop crop;
		obs_sceneitem_get_info2(item, &info);
		obs_sceneitem_get_crop(item, &crop);

		struct vec2 pos = info.pos;
		struct vec2 scale = info.scale;
		float rot = info.rot;
		uint32_t alignment = info.alignment;
		uint32_t bounds_type = (uint32_t)info.bounds_type;
		uint32_t bounds_alignment = info.bounds_alignment;
		bool crop_to_bounds = info.crop_to_bounds;
		struct vec2 bounds = info.bounds;

		obs_data_set_int(temp, "id", obs_sceneitem_get_id(item));
		obs_data_set_vec2(temp, "pos", &pos);
		obs_data_set_vec2(temp, "scale", &scale);
		obs_data_set_double(temp, "rot", (double)rot);
		obs_data_set_int(temp, "alignment", alignment);
		obs_data_set_int(temp, "bounds_type", bounds_type);
		obs_data_set_vec2(temp, "bounds", &bounds);
		obs_data_set_int(temp, "bounds_alignment", bounds_alignment);
		obs_data_set_bool(temp, "crop_to_bounds", crop_to_bounds);
		obs_data_set_int(temp, "top", crop.top);
		obs_data_set_int(temp, "bottom", crop.bottom);
		obs_data_set_int(temp, "left", crop.left);
		obs_data_set_int(temp, "right", crop.right);

		obs_data_array_push_back(item_ids, temp);

		obs_data_release(temp);
	}

	obs_source_t *item_source = obs_sceneitem_get_source(item);

	if (obs_source_is_group(item_source)) {
		obs_data_t *temp = obs_data_create();
		obs_data_array_t *nids = obs_data_array_create();

		obs_data_set_string(temp, "scene_name", obs_source_get_name(item_source));
		obs_data_set_string(temp, "scene_uuid", obs_source_get_uuid(item_source));
		obs_data_set_bool(temp, "is_group", true);
		obs_data_set_string(temp, "group_parent", obs_source_get_uuid(obs_scene_get_source(scene)));

		struct passthrough npass = {nids, pass->scenes_and_groups, pass->all_items};
		obs_sceneitem_group_enum_items(item, save_transform_states, (void *)&npass);

		obs_data_set_array(temp, "items", nids);

		obs_data_array_push_back(pass->scenes_and_groups, temp);

		obs_data_release(temp);
		obs_data_array_release(nids);
	}

	return true;
}

obs_data_t *obs_scene_save_transform_states(obs_scene_t *scene, bool all_items)
{
	obs_data_t *wrapper = obs_data_create();
	obs_data_array_t *scenes_and_groups = obs_data_array_create();
	obs_data_array_t *item_ids = obs_data_array_create();

	struct passthrough pass = {item_ids, scenes_and_groups, all_items};

	obs_data_t *temp = obs_data_create();

	obs_data_set_string(temp, "scene_name", obs_source_get_name(obs_scene_get_source(scene)));
	obs_data_set_string(temp, "scene_uuid", obs_source_get_uuid(obs_scene_get_source(scene)));
	obs_data_set_bool(temp, "is_group", false);

	obs_scene_enum_items(scene, save_transform_states, (void *)&pass);

	obs_data_set_array(temp, "items", item_ids);
	obs_data_array_push_back(scenes_and_groups, temp);

	obs_data_set_array(wrapper, "scenes_and_groups", scenes_and_groups);

	obs_data_array_release(item_ids);
	obs_data_array_release(scenes_and_groups);
	obs_data_release(temp);

	return wrapper;
}

void load_transform_states(obs_data_t *temp, void *vp_scene);
void load_transform_states(obs_data_t *temp, void *vp_scene)
{
	obs_scene_t *scene = (obs_scene_t *)vp_scene;
	int64_t id = obs_data_get_int(temp, "id");
	obs_sceneitem_t *item = obs_scene_find_sceneitem_by_id(scene, id);

	struct obs_transform_info info;
	struct obs_sceneitem_crop crop;
	obs_data_get_vec2(temp, "pos", &info.pos);
	obs_data_get_vec2(temp, "scale", &info.scale);
	info.rot = (float)obs_data_get_double(temp, "rot");
	info.alignment = (uint32_t)obs_data_get_int(temp, "alignment");
	info.bounds_type = valid_bounds_type(obs_data_get_int(temp, "bounds_type"));
	info.bounds_alignment = (uint32_t)obs_data_get_int(temp, "bounds_alignment");
	obs_data_get_vec2(temp, "bounds", &info.bounds);
	info.crop_to_bounds = obs_data_get_bool(temp, "crop_to_bounds");
	crop.top = (int)obs_data_get_int(temp, "top");
	crop.bottom = (int)obs_data_get_int(temp, "bottom");
	crop.left = (int)obs_data_get_int(temp, "left");
	crop.right = (int)obs_data_get_int(temp, "right");

	obs_sceneitem_defer_update_begin(item);

	obs_sceneitem_set_info2(item, &info);
	obs_sceneitem_set_crop(item, &crop);

	obs_sceneitem_defer_update_end(item);
}

void iterate_scenes_and_groups_transform_states(obs_data_t *data, void *vp);
void iterate_scenes_and_groups_transform_states(obs_data_t *data, void *vp)
{
	obs_data_array_t *items = obs_data_get_array(data, "items");
	obs_source_t *scene_source = obs_get_source_by_uuid(obs_data_get_string(data, "scene_uuid"));
	obs_scene_t *scene = obs_scene_from_source(scene_source);

	if (obs_data_get_bool(data, "is_group")) {
		obs_source_t *parent_source = obs_get_source_by_uuid(obs_data_get_string(data, "group_parent"));
		obs_scene_t *parent = obs_scene_from_source(parent_source);
		obs_sceneitem_t *group = obs_scene_get_group(parent, obs_data_get_string(data, "scene_name"));
		scene = obs_sceneitem_group_get_scene(group);

		obs_source_release(parent_source);
	}

	obs_data_array_enum(items, load_transform_states, (void *)scene);

	UNUSED_PARAMETER(vp);

	obs_data_array_release(items);
	obs_source_release(scene_source);
}

void obs_scene_load_transform_states(const char *data)
{
	obs_data_t *dat = obs_data_create_from_json(data);

	obs_data_array_t *scenes_and_groups = obs_data_get_array(dat, "scenes_and_groups");

	obs_data_array_enum(scenes_and_groups, iterate_scenes_and_groups_transform_states, NULL);

	obs_data_release(dat);
	obs_data_array_release(scenes_and_groups);
}
