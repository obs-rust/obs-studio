#include "libobs/build/libobs.pch.h"

static void execute_graphics_tasks(void);

void obs_graphics_tasks_execute(struct obs_core_video *video);
void obs_graphics_tasks_execute(struct obs_core_video *video)
{
	(void)video;
	execute_graphics_tasks();
}
