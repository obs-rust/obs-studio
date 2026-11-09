/* Unity group (BUILD.md): original graphics task drain, C reference only. */

#include "libobs/build/libobs.pch.h"

#include "libobs/obs-video-graphics-tasks.c.h"

void obs_graphics_tasks_execute(struct obs_core_video *video);
void obs_graphics_tasks_execute(struct obs_core_video *video)
{
	(void)video;
	execute_graphics_tasks();
}
