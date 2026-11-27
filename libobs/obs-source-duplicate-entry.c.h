#include "libobs/build/libobs.pch.h"

obs_source_t *obs_source_duplicate(obs_source_t *source, const char *new_name, bool create_private)
{
	obs_source_t *new_source;
	obs_data_t *settings;

	if (!obs_source_valid(source, "obs_source_duplicate"))
		return NULL;

	if (obs_source_duplicate_type_step((int)source->info.type) == OBS_SOURCE_DUPLICATE_ASK_SCENE) {
		obs_scene_t *scene = obs_scene_from_source(source);
		int step = obs_source_duplicate_scene_step(scene != NULL, create_private);
		if (step == OBS_SOURCE_DUPLICATE_REF) {
			return obs_source_get_ref(source);
		}
		if (step == OBS_SOURCE_DUPLICATE_ASK_GROUP) {
			scene = obs_group_from_source(source);
			step = obs_source_duplicate_group_step(scene != NULL);
		}
		if (step != OBS_SOURCE_DUPLICATE_SCENE_COPY)
			return NULL;

		obs_scene_t *new_scene = obs_scene_duplicate(
			scene, new_name,
			(enum obs_scene_duplicate_type)obs_source_duplicate_scene_type(create_private));
		obs_source_t *dup_source = obs_scene_get_source(new_scene);
		return dup_source;
	}

	if (obs_source_duplicate_flags_step(source->info.output_flags) == OBS_SOURCE_DUPLICATE_REF) {
		return obs_source_get_ref(source);
	}

	settings = obs_data_create();
	obs_data_apply(settings, source->context.settings);

	new_source = create_private ? obs_source_create_private(source->info.id, new_name, settings)
				    : obs_source_create(source->info.id, new_name, settings, NULL);

	if (obs_source_duplicate_created_step(new_source != NULL) != OBS_SOURCE_DUPLICATE_COPY_STATE) {
		obs_data_release(settings);
		return NULL;
	}

	new_source->audio_mixers = source->audio_mixers;
	new_source->sync_offset = obs_source_get_sync_offset(source);
	new_source->user_volume = obs_source_get_volume(source);
	os_atomic_set_bool(&new_source->user_muted, os_atomic_load_bool(&source->user_muted));
	new_source->volume = source->volume;
	new_source->muted = source->muted;
	new_source->flags = source->flags;

	obs_data_apply(new_source->private_settings, source->private_settings);

	if (obs_source_duplicate_copies_filters((int)source->info.type))
		obs_source_duplicate_filters(new_source, source, create_private);

	obs_data_release(settings);
	return new_source;
}
