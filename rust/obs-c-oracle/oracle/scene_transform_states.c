/* Exact extracted C policy, with isolated scene collaborators and real C data objects. */
#define obs_data_create oracle_obs_data_create
#define obs_data_create_from_json oracle_obs_data_create_from_json
#define obs_data_release oracle_obs_data_release
#define obs_data_set_string oracle_obs_data_set_string
#define obs_data_set_int oracle_obs_data_set_int
#define obs_data_set_double oracle_obs_data_set_double
#define obs_data_set_bool oracle_obs_data_set_bool
#define obs_data_set_vec2 oracle_obs_data_set_vec2
#define obs_data_set_array oracle_obs_data_set_array
#define obs_data_get_string oracle_obs_data_get_string
#define obs_data_get_int oracle_obs_data_get_int
#define obs_data_get_double oracle_obs_data_get_double
#define obs_data_get_bool oracle_obs_data_get_bool
#define obs_data_get_vec2 oracle_obs_data_get_vec2
#define obs_data_get_array oracle_obs_data_get_array
#define obs_data_array_create oracle_obs_data_array_create
#define obs_data_array_release oracle_obs_data_array_release
#define obs_data_array_push_back oracle_obs_data_array_push_back
#define obs_data_array_enum oracle_obs_data_array_enum
#define obs_data_array_item oracle_obs_data_array_item
#define obs_data_array_count oracle_obs_data_array_count
#define obs_data_get_json oracle_obs_data_get_json
#define obs_scene_get_source sts_obs_scene_get_source
#define obs_sceneitem_get_source sts_obs_sceneitem_get_source
#define obs_source_is_group sts_obs_source_is_group
#define obs_source_get_name sts_obs_source_get_name
#define obs_source_get_uuid sts_obs_source_get_uuid
#define obs_sceneitem_selected sts_obs_sceneitem_selected
#define obs_sceneitem_get_id sts_obs_sceneitem_get_id
#define obs_sceneitem_get_info2 sts_obs_sceneitem_get_info2
#define obs_sceneitem_get_crop sts_obs_sceneitem_get_crop
#define obs_get_source_by_uuid sts_obs_get_source_by_uuid
#define obs_scene_from_source sts_obs_scene_from_source
#define obs_scene_get_group sts_obs_scene_get_group
#define obs_sceneitem_group_get_scene sts_obs_sceneitem_group_get_scene
#define obs_source_release sts_obs_source_release
#define obs_scene_find_sceneitem_by_id sts_obs_scene_find_sceneitem_by_id
#define obs_sceneitem_defer_update_begin sts_obs_sceneitem_defer_update_begin
#define obs_sceneitem_defer_update_end sts_obs_sceneitem_defer_update_end
#define obs_sceneitem_set_info2 sts_obs_sceneitem_set_info2
#define obs_sceneitem_set_crop sts_obs_sceneitem_set_crop
#define obs_scene_enum_items sts_obs_scene_enum_items
#define obs_sceneitem_group_enum_items sts_obs_sceneitem_group_enum_items
#define save_transform_states oracle_sts_save_transform_states
#define obs_scene_save_transform_states oracle_sts_obs_scene_save_transform_states
#define load_transform_states oracle_sts_load_transform_states
#define iterate_scenes_and_groups_transform_states oracle_sts_iterate_scenes_and_groups_transform_states
#define obs_scene_load_transform_states oracle_sts_obs_scene_load_transform_states
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "obs.h"
#include "scene_transform_states.inc"

struct sts_source {
	const char *name;
	const char *uuid;
	unsigned kind;
	int refs;
};
struct sts_item {
	unsigned parent, source;
	int64_t id;
	bool selected;
	struct obs_transform_info info;
	struct obs_sceneitem_crop crop;
	unsigned deferred, signals;
};
/* The Rust differential module serializes access to this bounded fixture. */
static struct sts_source sts_sources[5];
static struct sts_item sts_items[7];
static unsigned sts_count;
static struct sts_source *sts_source(const void *p)
{
	return &sts_sources[(const struct sts_source *)p - sts_sources];
}
static struct sts_item *sts_item(const void *p)
{
	return &sts_items[(const struct sts_item *)p - sts_items];
}
obs_source_t *sts_obs_scene_get_source(const obs_scene_t *s)
{
	return s ? (obs_source_t *)sts_source(s) : NULL;
}
obs_source_t *sts_obs_sceneitem_get_source(const obs_sceneitem_t *i)
{
	return i ? (obs_source_t *)&sts_sources[sts_item(i)->source] : NULL;
}
bool sts_obs_source_is_group(const obs_source_t *s)
{
	return s && sts_source(s)->kind == 2;
}
const char *sts_obs_source_get_name(const obs_source_t *s)
{
	return s ? sts_source(s)->name : "";
}
const char *sts_obs_source_get_uuid(const obs_source_t *s)
{
	return s ? sts_source(s)->uuid : "";
}
bool sts_obs_sceneitem_selected(const obs_sceneitem_t *i)
{
	return i && sts_item(i)->selected;
}
int64_t sts_obs_sceneitem_get_id(const obs_sceneitem_t *i)
{
	return i ? sts_item(i)->id : 0;
}
void sts_obs_sceneitem_get_info2(const obs_sceneitem_t *i, struct obs_transform_info *v)
{
	if (i)
		*v = sts_item(i)->info;
}
void sts_obs_sceneitem_get_crop(const obs_sceneitem_t *i, struct obs_sceneitem_crop *v)
{
	if (i)
		*v = sts_item(i)->crop;
}
obs_source_t *sts_obs_get_source_by_uuid(const char *uuid)
{
	for (unsigned i = 1; i < 5; i++)
		if (!strcmp(sts_sources[i].uuid, uuid)) {
			sts_sources[i].refs++;
			return (obs_source_t *)&sts_sources[i];
		}
	return NULL;
}
obs_scene_t *sts_obs_scene_from_source(const obs_source_t *s)
{
	return s && sts_source(s)->kind == 1 ? (obs_scene_t *)sts_source(s) : NULL;
}
obs_sceneitem_t *sts_obs_scene_get_group(obs_scene_t *s, const char *name)
{
	if (!s)
		return NULL;
	for (unsigned i = 1; i <= sts_count; i++)
		if (&sts_sources[sts_items[i].parent] == sts_source(s) && sts_sources[sts_items[i].source].kind == 2 &&
		    !strcmp(sts_sources[sts_items[i].source].name, name))
			return (obs_sceneitem_t *)&sts_items[i];
	return NULL;
}
obs_scene_t *sts_obs_sceneitem_group_get_scene(const obs_sceneitem_t *i)
{
	return i ? (obs_scene_t *)&sts_sources[sts_item(i)->source] : NULL;
}
void sts_obs_source_release(obs_source_t *s)
{
	if (s)
		sts_source(s)->refs--;
}
obs_sceneitem_t *sts_obs_scene_find_sceneitem_by_id(obs_scene_t *s, int64_t id)
{
	if (!s)
		return NULL;
	for (unsigned i = 1; i <= sts_count; i++)
		if (&sts_sources[sts_items[i].parent] == sts_source(s) && sts_items[i].id == id)
			return (obs_sceneitem_t *)&sts_items[i];
	return NULL;
}
void sts_obs_sceneitem_defer_update_begin(obs_sceneitem_t *i)
{
	if (i)
		sts_item(i)->deferred++;
}
void sts_obs_sceneitem_defer_update_end(obs_sceneitem_t *i)
{
	if (i && --sts_item(i)->deferred == 0)
		sts_item(i)->signals++;
}
void sts_obs_sceneitem_set_info2(obs_sceneitem_t *i, const struct obs_transform_info *v)
{
	if (i)
		sts_item(i)->info = *v;
}
void sts_obs_sceneitem_set_crop(obs_sceneitem_t *i, const struct obs_sceneitem_crop *v)
{
	if (!i)
		return;
	struct obs_sceneitem_crop c = *v;
	if (c.left < 0)
		c.left = 0;
	if (c.top < 0)
		c.top = 0;
	if (c.right < 0)
		c.right = 0;
	if (c.bottom < 0)
		c.bottom = 0;
	sts_item(i)->crop = c;
}
void sts_obs_scene_enum_items(obs_scene_t *s, bool (*cb)(obs_scene_t *, obs_sceneitem_t *, void *), void *p)
{
	if (!s)
		return;
	for (unsigned i = 1; i <= sts_count; i++)
		if (&sts_sources[sts_items[i].parent] == sts_source(s) && !cb(s, (obs_sceneitem_t *)&sts_items[i], p))
			break;
}
void sts_obs_sceneitem_group_enum_items(obs_sceneitem_t *i, bool (*cb)(obs_scene_t *, obs_sceneitem_t *, void *),
					void *p)
{
	sts_obs_scene_enum_items(sts_obs_sceneitem_group_get_scene(i), cb, p);
}
static uint64_t sts_bits(float f)
{
	uint32_t n;
	memcpy(&n, &f, sizeof(n));
	return n;
}
void oracle_sts_run(bool all, bool groups, uint8_t selected, int64_t bounds, bool bad_id, bool bad_uuid, uint32_t seed,
		    char *json, size_t capacity, uint64_t *out);
void oracle_sts_run(bool all, bool groups, uint8_t selected, int64_t bounds, bool bad_id, bool bad_uuid, uint32_t seed,
		    char *json, size_t capacity, uint64_t *out)
{
	const char *names[] = {"", "snapshot scene", "outer", "inner", "snapshot leaf"};
	const char *uuids[] = {"uuid-0", "uuid-1", "uuid-2", "uuid-3", "uuid-4"};
	const unsigned kinds[] = {0, 1, 2, 2, 0}, parents[] = {0, 1, 1, 1, 2, 3, 2}, sources[] = {0, 4, 4, 2, 3, 4, 4};
	for (unsigned i = 0; i < 5; i++)
		sts_sources[i] = (struct sts_source){names[i], uuids[i], kinds[i], 1};
	memset(sts_items, 0, sizeof(sts_items));
	sts_count = groups ? 6 : 2;
	for (unsigned i = 1; i <= sts_count; i++) {
		struct sts_item *v = &sts_items[i];
		v->parent = parents[i];
		v->source = sources[i];
		v->id = (int64_t)i;
		v->selected = (selected & (1u << (i - 1))) != 0;
		v->info.pos = (struct vec2){.x = 37.25f + (float)(seed % 32), .y = -19.5f};
		v->info.rot = 31.5f + (float)(seed % 64);
		v->info.scale = (struct vec2){.x = -1.25f, .y = 2.5f};
		v->info.alignment = 10;
		v->info.bounds_type = OBS_BOUNDS_SCALE_OUTER;
		v->info.bounds_alignment = 5;
		v->info.bounds = (struct vec2){.x = 333.0f, .y = 222.0f};
		v->info.crop_to_bounds = true;
		v->crop = (struct obs_sceneitem_crop){3, 5, 7, 11};
	}
	obs_data_t *d = oracle_sts_obs_scene_save_transform_states((obs_scene_t *)&sts_sources[1], all);
	snprintf(json, capacity, "%s", obs_data_get_json(d));
	obs_data_array_t *records = obs_data_get_array(d, "scenes_and_groups");
	obs_data_t *root = obs_data_array_item(records, obs_data_array_count(records) - 1);
	obs_data_array_t *items = obs_data_get_array(root, "items");
	for (size_t i = 0; i < obs_data_array_count(items); i++) {
		obs_data_t *v = obs_data_array_item(items, i);
		obs_data_set_int(v, "bounds_type", bounds);
		if (bad_id)
			obs_data_set_int(v, "id", INT64_MAX);
		obs_data_release(v);
	}
	if (bad_uuid)
		obs_data_set_string(root, "scene_uuid", "missing scene uuid");
	/* Group UUID is deliberately missing: parent UUID/name remains authoritative. */
	if (groups) {
		obs_data_t *outer = obs_data_array_item(records, 1);
		obs_data_set_string(outer, "scene_uuid", "missing group uuid");
		obs_data_release(outer);
	}
	for (unsigned i = 1; i <= sts_count; i++) {
		memset(&sts_items[i].info, 0, sizeof(sts_items[i].info));
		memset(&sts_items[i].crop, 0, sizeof(sts_items[i].crop));
	}
	sts_items[1].deferred = seed & 1;
	sts_sources[1].name = "renamed scene";
	oracle_sts_obs_scene_load_transform_states(obs_data_get_json(d));
	memset(out, 0, 128 * sizeof(*out));
	for (unsigned i = 1; i <= sts_count; i++) {
		struct sts_item *v = &sts_items[i];
		struct obs_transform_info *t = &v->info;
		uint64_t *o = out + (i - 1) * 18;
		o[0] = sts_bits(t->pos.x);
		o[1] = sts_bits(t->pos.y);
		o[2] = sts_bits(t->rot);
		o[3] = sts_bits(t->scale.x);
		o[4] = sts_bits(t->scale.y);
		o[5] = t->alignment;
		o[6] = t->bounds_type;
		o[7] = t->bounds_alignment;
		o[8] = sts_bits(t->bounds.x);
		o[9] = sts_bits(t->bounds.y);
		o[10] = t->crop_to_bounds;
		o[11] = (uint64_t)v->crop.left;
		o[12] = (uint64_t)v->crop.top;
		o[13] = (uint64_t)v->crop.right;
		o[14] = (uint64_t)v->crop.bottom;
		o[15] = v->deferred;
		o[16] = v->signals;
		o[17] = v->selected;
	}
	for (unsigned i = 0; i < 5; i++)
		out[108 + i] = (uint64_t)sts_sources[i].refs;
	obs_data_array_release(items);
	obs_data_release(root);
	obs_data_array_release(records);
	obs_data_release(d);
}
void oracle_sts_layout(size_t *out);
void oracle_sts_layout(size_t *out)
{
	const size_t values[] = {sizeof(struct obs_transform_info),
				 _Alignof(struct obs_transform_info),
				 offsetof(struct obs_transform_info, pos),
				 offsetof(struct obs_transform_info, rot),
				 offsetof(struct obs_transform_info, scale),
				 offsetof(struct obs_transform_info, alignment),
				 offsetof(struct obs_transform_info, bounds_type),
				 offsetof(struct obs_transform_info, bounds_alignment),
				 offsetof(struct obs_transform_info, bounds),
				 offsetof(struct obs_transform_info, crop_to_bounds),
				 sizeof(struct obs_sceneitem_crop),
				 _Alignof(struct obs_sceneitem_crop),
				 offsetof(struct obs_sceneitem_crop, left),
				 offsetof(struct obs_sceneitem_crop, top),
				 offsetof(struct obs_sceneitem_crop, right),
				 offsetof(struct obs_sceneitem_crop, bottom),
				 sizeof(struct vec2),
				 _Alignof(struct vec2),
				 offsetof(struct vec2, x),
				 offsetof(struct vec2, y)};
	memcpy(out, values, sizeof(values));
}
#undef obs_data_create
#undef obs_data_create_from_json
#undef obs_data_release
#undef obs_data_set_string
#undef obs_data_set_int
#undef obs_data_set_double
#undef obs_data_set_bool
#undef obs_data_set_vec2
#undef obs_data_set_array
#undef obs_data_get_string
#undef obs_data_get_int
#undef obs_data_get_double
#undef obs_data_get_bool
#undef obs_data_get_vec2
#undef obs_data_get_array
#undef obs_data_array_create
#undef obs_data_array_release
#undef obs_data_array_push_back
#undef obs_data_array_enum
#undef obs_data_array_item
#undef obs_data_array_count
#undef obs_data_get_json
#undef obs_scene_get_source
#undef obs_sceneitem_get_source
#undef obs_source_is_group
#undef obs_source_get_name
#undef obs_source_get_uuid
#undef obs_sceneitem_selected
#undef obs_sceneitem_get_id
#undef obs_sceneitem_get_info2
#undef obs_sceneitem_get_crop
#undef obs_get_source_by_uuid
#undef obs_scene_from_source
#undef obs_scene_get_group
#undef obs_sceneitem_group_get_scene
#undef obs_source_release
#undef obs_scene_find_sceneitem_by_id
#undef obs_sceneitem_defer_update_begin
#undef obs_sceneitem_defer_update_end
#undef obs_sceneitem_set_info2
#undef obs_sceneitem_set_crop
#undef obs_scene_enum_items
#undef obs_sceneitem_group_enum_items
#undef save_transform_states
#undef obs_scene_save_transform_states
#undef load_transform_states
#undef iterate_scenes_and_groups_transform_states
#undef obs_scene_load_transform_states
