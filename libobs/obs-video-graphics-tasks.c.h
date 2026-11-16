#include "libobs/build/libobs.pch.h"

/* Private bridge to obs_video_graphics_tasks_execute()
 * (obs-video-graphics-tasks.h): these accessors give it the graphics task
 * queue, which stays C-owned. */
static void graphics_tasks_lock(struct obs_core *core)
{
	pthread_mutex_lock(&core->video.task_mutex);
}

static void graphics_tasks_unlock(struct obs_core *core)
{
	pthread_mutex_unlock(&core->video.task_mutex);
}

static bool graphics_tasks_pending(struct obs_core *core)
{
	return core->video.tasks.size != 0;
}

static void graphics_tasks_pop(struct obs_core *core, struct obs_task_info *info)
{
	deque_pop_front(&core->video.tasks, info, sizeof(*info));
}

static inline void execute_graphics_tasks(void)
{
	obs_video_graphics_tasks_execute(obs, graphics_tasks_lock, graphics_tasks_unlock, graphics_tasks_pending,
					 graphics_tasks_pop);
}
