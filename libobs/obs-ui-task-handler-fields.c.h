#include "libobs/build/libobs.pch.h"

_Static_assert(OBS_TASK_UI == 0, "Rust UI task tag must match the public enum");

struct obs_core *obs_core_ui_task_handler_core(void)
{
	return obs;
}

obs_task_handler_t *obs_core_ui_task_handler_field(struct obs_core *core)
{
	return &core->ui_task_handler;
}
