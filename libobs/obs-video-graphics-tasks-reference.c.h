#include "libobs/build/libobs.pch.h"

/* The original graphics task drain, for the C reference and the Tier 3
 * oracle. Its body is the C's, verbatim, over the core it is given. */
#define obs graphics_tasks_core
static inline void execute_graphics_tasks_reference(struct obs_core *obs)
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
#undef obs

void obs_video_graphics_tasks_execute(struct obs_core *core, void (*lock)(struct obs_core *core),
				      void (*unlock)(struct obs_core *core), bool (*pending)(struct obs_core *core),
				      void (*pop)(struct obs_core *core, struct obs_task_info *info))
{
	UNUSED_PARAMETER(lock);
	UNUSED_PARAMETER(unlock);
	UNUSED_PARAMETER(pending);
	UNUSED_PARAMETER(pop);
	execute_graphics_tasks_reference(core);
}
