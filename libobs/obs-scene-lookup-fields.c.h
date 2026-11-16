#include "libobs/build/libobs.pch.h"

/* Private adapter declarations; public OBS headers and layouts stay unchanged. */
struct obs_scene_item *obs_scene_lookup_lock(struct obs_scene *scene);
void obs_scene_lookup_unlock(struct obs_scene *scene);
struct obs_scene_item *obs_scene_lookup_next(const struct obs_scene_item *item);
const char *obs_scene_lookup_name(const struct obs_scene_item *item);
int64_t obs_scene_lookup_id(const struct obs_scene_item *item);
bool obs_scene_lookup_is_group(const struct obs_scene_item *item);
struct obs_scene *obs_scene_lookup_group(const struct obs_scene_item *item);

struct obs_scene_item *obs_scene_lookup_lock(struct obs_scene *scene)
{
	pthread_mutex_lock(&scene->video_mutex);
	pthread_mutex_lock(&scene->audio_mutex);
	return scene->first_item;
}

void obs_scene_lookup_unlock(struct obs_scene *scene)
{
	pthread_mutex_unlock(&scene->audio_mutex);
	pthread_mutex_unlock(&scene->video_mutex);
}

struct obs_scene_item *obs_scene_lookup_next(const struct obs_scene_item *item)
{
	return item->next;
}

const char *obs_scene_lookup_name(const struct obs_scene_item *item)
{
	return item->source->context.name;
}

int64_t obs_scene_lookup_id(const struct obs_scene_item *item)
{
	return item->id;
}

bool obs_scene_lookup_is_group(const struct obs_scene_item *item)
{
	return item->is_group;
}

struct obs_scene *obs_scene_lookup_group(const struct obs_scene_item *item)
{
	return item->source->context.data;
}
