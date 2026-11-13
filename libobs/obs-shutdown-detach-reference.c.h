#include "libobs/build/libobs.pch.h"

/* The original shutdown detachment, for the C reference and the Tier 3
 * oracle. Its bodies are the C's, verbatim. */
static bool obs_shutdown_remove_scene_item(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	UNUSED_PARAMETER(scene);
	UNUSED_PARAMETER(param);
	obs_sceneitem_remove(item);
	return true;
}

static void obs_shutdown_detach_scene_items_reference(struct obs_core_data *data)
{
	DARRAY(obs_source_t *) sources;
	da_init(sources);
	struct obs_context_data *ctx, *tmp;

	/* A scene can release the last reference of another table member.
	 * Snapshot under the table lock and retain every source until all scene
	 * and group items have been detached, including nested groups (#3458). */
	pthread_mutex_lock(&data->sources_mutex);
	HASH_ITER (hh_uuid, *(struct obs_context_data **)&data->sources, ctx, tmp) {
		obs_source_t *source = obs_source_get_ref((obs_source_t *)ctx);
		if (source)
			da_push_back(sources, &source);
	}
	pthread_mutex_unlock(&data->sources_mutex);

	for (size_t i = 0; i < sources.num; i++) {
		obs_scene_t *scene = obs_scene_from_source(sources.array[i]);
		if (!scene)
			scene = obs_group_from_source(sources.array[i]);
		if (scene)
			obs_scene_enum_items(scene, obs_shutdown_remove_scene_item, NULL);
	}
	for (size_t i = 0; i < sources.num; i++)
		obs_source_release(sources.array[i]);
	da_free(sources);

	/* Sources owned only by an item may now have queued their destruction.
	 * Finish that work before walking the tables to reclaim host leaks. */
	while (os_task_queue_wait(obs->destruction_task_thread))
		;
}

void obs_shutdown_detach_scene_items_run(struct obs_core_data *data, const struct obs_shutdown_detach_calls *calls)
{
	UNUSED_PARAMETER(calls);
	obs_shutdown_detach_scene_items_reference(data);
}
