/* The original graphics task drain (the C reference) and the production
 * bridge's queue accessors, over the real private core layout. */
#define obs_video_graphics_tasks_execute oracle_obs_video_graphics_tasks_execute
#include "libobs/obs-video-graphics-tasks-reference.c.h"
#include "libobs/obs-video-graphics-tasks.c.h"
#undef obs_video_graphics_tasks_execute
#include <stdlib.h>

void *oracle_graphics_tasks_new(void);
void oracle_graphics_tasks_free(void *core);
bool oracle_graphics_tasks_push(void *core, void (*task)(void *param), void *param);
size_t oracle_graphics_tasks_size(void *core);
bool oracle_graphics_tasks_locked(void *core);
void oracle_graphics_tasks_original(void *core);
void oracle_graphics_tasks_lock(void *core);
void oracle_graphics_tasks_unlock(void *core);
bool oracle_graphics_tasks_pending(void *core);
void oracle_graphics_tasks_pop(void *core, struct obs_task_info *info);

/* A core with an empty graphics task queue and a real task_mutex. */
void *oracle_graphics_tasks_new(void)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	if (!core)
		return NULL;
	if (pthread_mutex_init(&core->video.task_mutex, NULL) != 0) {
		free(core);
		return NULL;
	}
	return core;
}

void oracle_graphics_tasks_free(void *opaque)
{
	struct obs_core *core = opaque;
	deque_free(&core->video.tasks);
	pthread_mutex_destroy(&core->video.task_mutex);
	free(core);
}

/* obs_queue_task()'s graphics append from another producer: take
 * task_mutex, push, release. It uses trylock, so a drain that still holds
 * the mutex while a task runs makes the push fail (false) instead of
 * deadlocking the test. */
bool oracle_graphics_tasks_push(void *opaque, void (*task)(void *param), void *param)
{
	struct obs_core *core = opaque;
	struct obs_task_info info = {task, param};
	if (pthread_mutex_trylock(&core->video.task_mutex) != 0)
		return false;
	deque_push_back(&core->video.tasks, &info, sizeof(info));
	pthread_mutex_unlock(&core->video.task_mutex);
	return true;
}

size_t oracle_graphics_tasks_size(void *opaque)
{
	struct obs_core *core = opaque;
	return core->video.tasks.size / sizeof(struct obs_task_info);
}

/* Whether task_mutex is held (by anyone, this thread included). */
bool oracle_graphics_tasks_locked(void *opaque)
{
	struct obs_core *core = opaque;
	if (pthread_mutex_trylock(&core->video.task_mutex) != 0)
		return true;
	pthread_mutex_unlock(&core->video.task_mutex);
	return false;
}

void oracle_graphics_tasks_original(void *core)
{
	oracle_obs_video_graphics_tasks_execute(core, NULL, NULL, NULL, NULL);
}

void oracle_graphics_tasks_lock(void *core)
{
	graphics_tasks_lock(core);
}

void oracle_graphics_tasks_unlock(void *core)
{
	graphics_tasks_unlock(core);
}

bool oracle_graphics_tasks_pending(void *core)
{
	return graphics_tasks_pending(core);
}

void oracle_graphics_tasks_pop(void *core, struct obs_task_info *info)
{
	graphics_tasks_pop(core, info);
}
