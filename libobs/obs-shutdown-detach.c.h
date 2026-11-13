#include "libobs/build/libobs.pch.h"

/* Private bridge to obs_shutdown_detach_scene_items_run()
 * (obs-shutdown-detach.h): these calls give it the source table, its mutex
 * and the destruction queue, which stay C-owned. */
static void shutdown_detach_lock(struct obs_core_data *data)
{
	pthread_mutex_lock(&data->sources_mutex);
}

static void shutdown_detach_unlock(struct obs_core_data *data)
{
	pthread_mutex_unlock(&data->sources_mutex);
}

static void shutdown_detach_each_source(struct obs_core_data *data, void (*visit)(void *param, obs_source_t *source),
					void *param)
{
	struct obs_context_data *ctx, *tmp;
	HASH_ITER (hh_uuid, *(struct obs_context_data **)&data->sources, ctx, tmp) {
		visit(param, (obs_source_t *)ctx);
	}
}

static bool shutdown_detach_wait_destruction(void)
{
	return os_task_queue_wait(obs->destruction_task_thread);
}

static const struct obs_shutdown_detach_calls shutdown_detach_calls = {
	.lock = shutdown_detach_lock,
	.unlock = shutdown_detach_unlock,
	.each_source = shutdown_detach_each_source,
	.get_ref = obs_source_get_ref,
	.scene_from_source = obs_scene_from_source,
	.group_from_source = obs_group_from_source,
	.enum_items = obs_scene_enum_items,
	.item_remove = obs_sceneitem_remove,
	.release = obs_source_release,
	.wait_destruction = shutdown_detach_wait_destruction,
};

static inline void obs_shutdown_detach_scene_items(struct obs_core_data *data)
{
	obs_shutdown_detach_scene_items_run(data, &shutdown_detach_calls);
}
