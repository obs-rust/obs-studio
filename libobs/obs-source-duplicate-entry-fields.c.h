#include "libobs/build/libobs.pch.h"

/* What obs_source_duplicate() reads, writes and calls (the obs-source shim).
 * The caller rejects a NULL source and passes the source it was given, the
 * new source it created (dst) or a scene the lookups or the copy returned;
 * each call is one read, one write or one libobs call. */
int obs_source_duplicate_entry_type(const obs_source_t *source);
int obs_source_duplicate_entry_type(const obs_source_t *source)
{
	return (int)source->info.type;
}

obs_scene_t *obs_source_duplicate_entry_scene(obs_source_t *source);
obs_scene_t *obs_source_duplicate_entry_scene(obs_source_t *source)
{
	return obs_scene_from_source(source);
}

obs_scene_t *obs_source_duplicate_entry_group(obs_source_t *source);
obs_scene_t *obs_source_duplicate_entry_group(obs_source_t *source)
{
	return obs_group_from_source(source);
}

obs_source_t *obs_source_duplicate_entry_get_ref(obs_source_t *source);
obs_source_t *obs_source_duplicate_entry_get_ref(obs_source_t *source)
{
	return obs_source_get_ref(source);
}

obs_scene_t *obs_source_duplicate_entry_scene_duplicate(obs_scene_t *scene, const char *new_name, int type);
obs_scene_t *obs_source_duplicate_entry_scene_duplicate(obs_scene_t *scene, const char *new_name, int type)
{
	return obs_scene_duplicate(scene, new_name, (enum obs_scene_duplicate_type)type);
}

obs_source_t *obs_source_duplicate_entry_scene_source(obs_scene_t *scene);
obs_source_t *obs_source_duplicate_entry_scene_source(obs_scene_t *scene)
{
	return obs_scene_get_source(scene);
}

uint32_t obs_source_duplicate_entry_output_flags(const obs_source_t *source);
uint32_t obs_source_duplicate_entry_output_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

obs_data_t *obs_source_duplicate_entry_settings_create(void);
obs_data_t *obs_source_duplicate_entry_settings_create(void)
{
	return obs_data_create();
}

void obs_source_duplicate_entry_settings_apply(obs_data_t *settings, const obs_source_t *source);
void obs_source_duplicate_entry_settings_apply(obs_data_t *settings, const obs_source_t *source)
{
	obs_data_apply(settings, source->context.settings);
}

obs_source_t *obs_source_duplicate_entry_create(const obs_source_t *source, const char *new_name, obs_data_t *settings,
						bool create_private);
obs_source_t *obs_source_duplicate_entry_create(const obs_source_t *source, const char *new_name, obs_data_t *settings,
						bool create_private)
{
	return create_private ? obs_source_create_private(source->info.id, new_name, settings)
			      : obs_source_create(source->info.id, new_name, settings, NULL);
}

void obs_source_duplicate_entry_settings_release(obs_data_t *settings);
void obs_source_duplicate_entry_settings_release(obs_data_t *settings)
{
	obs_data_release(settings);
}

uint32_t obs_source_duplicate_entry_audio_mixers(const obs_source_t *source);
uint32_t obs_source_duplicate_entry_audio_mixers(const obs_source_t *source)
{
	return source->audio_mixers;
}

void obs_source_duplicate_entry_set_audio_mixers(obs_source_t *dst, uint32_t value);
void obs_source_duplicate_entry_set_audio_mixers(obs_source_t *dst, uint32_t value)
{
	dst->audio_mixers = value;
}

int64_t obs_source_duplicate_entry_sync_offset(const obs_source_t *source);
int64_t obs_source_duplicate_entry_sync_offset(const obs_source_t *source)
{
	return obs_source_get_sync_offset(source);
}

void obs_source_duplicate_entry_set_sync_offset(obs_source_t *dst, int64_t value);
void obs_source_duplicate_entry_set_sync_offset(obs_source_t *dst, int64_t value)
{
	dst->sync_offset = value;
}

float obs_source_duplicate_entry_user_volume(const obs_source_t *source);
float obs_source_duplicate_entry_user_volume(const obs_source_t *source)
{
	return obs_source_get_volume(source);
}

void obs_source_duplicate_entry_set_user_volume(obs_source_t *dst, float value);
void obs_source_duplicate_entry_set_user_volume(obs_source_t *dst, float value)
{
	dst->user_volume = value;
}

bool obs_source_duplicate_entry_user_muted(const obs_source_t *source);
bool obs_source_duplicate_entry_user_muted(const obs_source_t *source)
{
	return os_atomic_load_bool(&source->user_muted);
}

void obs_source_duplicate_entry_set_user_muted(obs_source_t *dst, bool value);
void obs_source_duplicate_entry_set_user_muted(obs_source_t *dst, bool value)
{
	os_atomic_set_bool(&dst->user_muted, value);
}

float obs_source_duplicate_entry_volume(const obs_source_t *source);
float obs_source_duplicate_entry_volume(const obs_source_t *source)
{
	return source->volume;
}

void obs_source_duplicate_entry_set_volume(obs_source_t *dst, float value);
void obs_source_duplicate_entry_set_volume(obs_source_t *dst, float value)
{
	dst->volume = value;
}

bool obs_source_duplicate_entry_muted(const obs_source_t *source);
bool obs_source_duplicate_entry_muted(const obs_source_t *source)
{
	return source->muted;
}

void obs_source_duplicate_entry_set_muted(obs_source_t *dst, bool value);
void obs_source_duplicate_entry_set_muted(obs_source_t *dst, bool value)
{
	dst->muted = value;
}

uint32_t obs_source_duplicate_entry_flags(const obs_source_t *source);
uint32_t obs_source_duplicate_entry_flags(const obs_source_t *source)
{
	return source->flags;
}

void obs_source_duplicate_entry_set_flags(obs_source_t *dst, uint32_t value);
void obs_source_duplicate_entry_set_flags(obs_source_t *dst, uint32_t value)
{
	dst->flags = value;
}

void obs_source_duplicate_entry_private_settings_apply(obs_source_t *dst, const obs_source_t *source);
void obs_source_duplicate_entry_private_settings_apply(obs_source_t *dst, const obs_source_t *source)
{
	obs_data_apply(dst->private_settings, source->private_settings);
}

void obs_source_duplicate_entry_filters(obs_source_t *dst, obs_source_t *source, bool create_private);
void obs_source_duplicate_entry_filters(obs_source_t *dst, obs_source_t *source, bool create_private)
{
	obs_source_duplicate_filters(dst, source, create_private);
}
