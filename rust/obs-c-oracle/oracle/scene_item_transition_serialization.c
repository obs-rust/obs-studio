/* Original serialization orchestration, with isolated, reference-counted source
 * collaborators. Keep data calls in the C oracle too: MSVC may select this
 * archive member in obs-util's harness, which does not link Rust obs-data. */
#define obs_data_create oracle_obs_data_create
#define obs_data_addref oracle_obs_data_addref
#define obs_data_release oracle_obs_data_release
#define obs_data_set_string oracle_obs_data_set_string
#define obs_data_set_int oracle_obs_data_set_int
#define obs_data_set_obj oracle_obs_data_set_obj
#define obs_data_get_string oracle_obs_data_get_string
#define obs_data_get_int oracle_obs_data_get_int
#define obs_data_get_obj oracle_obs_data_get_obj
#define obs_data_has_user_value oracle_obs_data_has_user_value
#define obs_sceneitem_transition_load oracle_obs_sceneitem_transition_load
#define obs_sceneitem_transition_save oracle_obs_sceneitem_transition_save
#define obs_source_create_private oracle_sits_create
#define obs_source_release oracle_sits_release
#define obs_source_get_unversioned_id oracle_sits_unversioned
#define obs_source_get_id oracle_sits_id
#define obs_source_get_name oracle_sits_name
#define obs_source_get_settings oracle_sits_settings
#define obs_sceneitem_set_transition oracle_sits_set
#define obs_sceneitem_set_transition_duration oracle_sits_duration
#include "obs-scene-item-transition-serialization.c.h"
#undef obs_sceneitem_transition_load
#undef obs_sceneitem_transition_save
#undef obs_source_create_private
#undef obs_source_release
#undef obs_source_get_unversioned_id
#undef obs_source_get_id
#undef obs_source_get_name
#undef obs_source_get_settings
#undef obs_sceneitem_set_transition
#undef obs_sceneitem_set_transition_duration

struct sits_source {
	struct obs_source source;
	unsigned refs;
	char id[32];
	char name[32];
};
/* Access is serialized by the Rust parity module's mutex. */
static struct sits_source sits_sources[3];
static unsigned sits_created, sits_destroyed;
obs_source_t *oracle_sits_create(const char *id, const char *name, obs_data_t *settings)
{
	struct sits_source *s = &sits_sources[sits_created++];
	memset(s, 0, sizeof(*s));
	s->refs = 1;
	snprintf(s->id, sizeof(s->id), "%s", id);
	snprintf(s->name, sizeof(s->name), "%s", name);
	s->source.info.id = s->id;
	s->source.info.unversioned_id = s->id;
	s->source.context.name = s->name;
	s->source.context.settings = settings;
	obs_data_addref(settings);
	return &s->source;
}
void oracle_sits_release(obs_source_t *source)
{
	if (source && --((struct sits_source *)source)->refs == 0) {
		sits_destroyed++;
		obs_data_release(source->context.settings);
	}
}
const char *oracle_sits_unversioned(const obs_source_t *source)
{
	return source->info.unversioned_id;
}
const char *oracle_sits_id(const obs_source_t *source)
{
	return source->info.id;
}
const char *oracle_sits_name(const obs_source_t *source)
{
	return source->context.name;
}
obs_data_t *oracle_sits_settings(const obs_source_t *source)
{
	obs_data_addref(source->context.settings);
	return source->context.settings;
}
void oracle_sits_set(obs_sceneitem_t *item, bool show, obs_source_t *source)
{
	obs_source_t **slot = show ? &item->show_transition : &item->hide_transition;
	oracle_sits_release(*slot);
	*slot = source;
	if (source)
		((struct sits_source *)source)->refs++;
}
void oracle_sits_duration(obs_sceneitem_t *item, bool show, uint32_t duration)
{
	if (show)
		item->show_transition_duration = duration;
	else
		item->hide_transition_duration = duration;
}
void oracle_sits_run(int64_t duration, bool show, bool has_id, bool initial_show, bool initial_hide, bool null_item,
		     bool null_data, uint64_t *out);
void oracle_sits_run(int64_t duration, bool show, bool has_id, bool initial_show, bool initial_hide, bool null_item,
		     bool null_data, uint64_t *out)
{
	sits_created = sits_destroyed = 0;
	struct obs_scene_item item = {0};
	item.show_transition_duration = 11;
	item.hide_transition_duration = 22;
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "marker", "kept");
	for (unsigned i = 0; i < 2; i++) {
		if (i ? initial_show : initial_hide) {
			obs_source_t *s = oracle_sits_create("old", "Previous", settings);
			oracle_sits_set(&item, i != 0, s);
			oracle_sits_release(s);
		}
	}
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "id", has_id ? "new" : "");
	obs_data_set_string(data, "versioned_id", "ignored");
	obs_data_set_string(data, "name", "Loaded");
	obs_data_set_obj(data, "transition", settings);
	obs_data_set_int(data, "duration", duration);
	oracle_obs_sceneitem_transition_load(null_item ? NULL : &item, null_data ? NULL : data, show);
	out[0] = item.hide_transition_duration;
	out[1] = item.show_transition_duration;
	out[2] = item.hide_transition != NULL;
	out[3] = item.show_transition != NULL;
	out[4] = sits_created;
	out[5] = sits_destroyed;
	out[6] = 0;
	for (unsigned i = 0; i < sits_created; i++)
		out[6] += sits_sources[i].refs;
	for (unsigned i = 0; i < 2; i++) {
		obs_source_t *source = i ? item.show_transition : item.hide_transition;
		obs_data_t *saved = oracle_obs_sceneitem_transition_save(&item, i != 0);
		uint64_t flags = 0;
		if (source) {
			flags |= !strcmp(obs_data_get_string(saved, "id"), source->info.unversioned_id);
			flags |= (uint64_t)!strcmp(obs_data_get_string(saved, "versioned_id"), source->info.id) << 1;
			flags |= (uint64_t)!strcmp(obs_data_get_string(saved, "name"), source->context.name) << 2;
			obs_data_t *s = obs_data_get_obj(saved, "transition");
			flags |= (uint64_t)(s == settings) << 3;
			flags |= (uint64_t)!strcmp(obs_data_get_string(s, "marker"), "kept") << 4;
			obs_data_release(s);
		} else {
			flags = (uint64_t)!obs_data_has_user_value(saved, "id") |
				(uint64_t)!obs_data_has_user_value(saved, "versioned_id") << 1 |
				(uint64_t)!obs_data_has_user_value(saved, "name") << 2 |
				(uint64_t)!obs_data_has_user_value(saved, "transition") << 3;
		}
		out[7 + i] = flags;
		out[9 + i] = (uint64_t)obs_data_get_int(saved, "duration");
		obs_data_release(saved);
	}
	oracle_sits_set(&item, true, NULL);
	oracle_sits_set(&item, false, NULL);
	obs_data_release(data);
	obs_data_release(settings);
}

#undef obs_data_create
#undef obs_data_addref
#undef obs_data_release
#undef obs_data_set_string
#undef obs_data_set_int
#undef obs_data_set_obj
#undef obs_data_get_string
#undef obs_data_get_int
#undef obs_data_get_obj
#undef obs_data_has_user_value
