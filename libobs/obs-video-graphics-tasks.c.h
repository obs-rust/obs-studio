#include "libobs/build/libobs.pch.h"

static void execute_graphics_tasks(void)
{
	struct obs_core_video *video = &obs->video;
	pthread_mutex_lock(&video->task_mutex);
	while (video->tasks.size) {
		struct obs_task_info info;
		deque_pop_front(&video->tasks, &info, sizeof(info));
		pthread_mutex_unlock(&video->task_mutex);
		info.task(info.param);
		pthread_mutex_lock(&video->task_mutex);
	}
	pthread_mutex_unlock(&video->task_mutex);
}
