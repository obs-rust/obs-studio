#include "libobs/build/libobs.pch.h"

/* Private adapters of the Rust source_muted() and get_source_volume()
 * (obs-internal.h, no EXPORT; obs-source::ffi::source_muted_volume, #5853),
 * each one read or write of a field of obs-source-muted-volume.c.h. The key
 * is 0 for push-to-mute, 1 for push-to-talk. The callers pass a live
 * source. The pressed flags are written by the hotkey thread and read here
 * without atomics, as before; user_muted is loaded atomically. */
bool obs_source_muted_volume_key_enabled(const obs_source_t *source, int key);
bool obs_source_muted_volume_key_enabled(const obs_source_t *source, int key)
{
	return key ? source->push_to_talk_enabled : source->push_to_mute_enabled;
}

bool obs_source_muted_volume_user_pressed(const obs_source_t *source, int key);
bool obs_source_muted_volume_user_pressed(const obs_source_t *source, int key)
{
	return key ? source->user_push_to_talk_pressed : source->user_push_to_mute_pressed;
}

bool obs_source_muted_volume_pressed(const obs_source_t *source, int key);
bool obs_source_muted_volume_pressed(const obs_source_t *source, int key)
{
	return key ? source->push_to_talk_pressed : source->push_to_mute_pressed;
}

uint64_t obs_source_muted_volume_delay(const obs_source_t *source, int key);
uint64_t obs_source_muted_volume_delay(const obs_source_t *source, int key)
{
	return key ? source->push_to_talk_delay : source->push_to_mute_delay;
}

uint64_t obs_source_muted_volume_stop_time(const obs_source_t *source, int key);
uint64_t obs_source_muted_volume_stop_time(const obs_source_t *source, int key)
{
	return key ? source->push_to_talk_stop_time : source->push_to_mute_stop_time;
}

void obs_source_muted_volume_set_stop_time(obs_source_t *source, int key, uint64_t stop_time);
void obs_source_muted_volume_set_stop_time(obs_source_t *source, int key, uint64_t stop_time)
{
	if (key)
		source->push_to_talk_stop_time = stop_time;
	else
		source->push_to_mute_stop_time = stop_time;
}

bool obs_source_muted_volume_enabled(const obs_source_t *source);
bool obs_source_muted_volume_enabled(const obs_source_t *source)
{
	return source->enabled;
}

bool obs_source_muted_volume_user_muted(const obs_source_t *source);
bool obs_source_muted_volume_user_muted(const obs_source_t *source)
{
	return os_atomic_load_bool(&source->user_muted);
}

bool obs_source_muted_volume_muted(const obs_source_t *source);
bool obs_source_muted_volume_muted(const obs_source_t *source)
{
	return source->muted;
}

float obs_source_muted_volume_volume(const obs_source_t *source);
float obs_source_muted_volume_volume(const obs_source_t *source)
{
	return source->volume;
}
