#include "libobs/build/libobs.pch.h"

void obs_graphics_tasks_lock(struct obs_core_video *video);
void obs_graphics_tasks_unlock(struct obs_core_video *video);
bool obs_graphics_tasks_pending(struct obs_core_video *video);
void obs_graphics_tasks_pop(struct obs_core_video *video, struct obs_task_info *info);

void obs_graphics_tasks_lock(struct obs_core_video *video)
{
	pthread_mutex_lock(&video->task_mutex);
}

void obs_graphics_tasks_unlock(struct obs_core_video *video)
{
	pthread_mutex_unlock(&video->task_mutex);
}

bool obs_graphics_tasks_pending(struct obs_core_video *video)
{
	return video->tasks.size != 0;
}

void obs_graphics_tasks_pop(struct obs_core_video *video, struct obs_task_info *info)
{
	deque_pop_front(&video->tasks, info, sizeof(*info));
}
