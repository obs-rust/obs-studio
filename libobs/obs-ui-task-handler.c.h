#include "libobs/build/libobs.pch.h"

static THREAD_LOCAL bool is_ui_thread = false;

static void set_ui_thread(void *unused)
{
	is_ui_thread = true;
	UNUSED_PARAMETER(unused);
}

void obs_set_ui_task_handler(obs_task_handler_t handler)
{
	obs->ui_task_handler = handler;
	obs_queue_task(OBS_TASK_UI, set_ui_thread, NULL, false);
}

/* Private bridge for the unported task-thread query. */
bool obs_core_is_ui_thread(void)
{
	return is_ui_thread;
}
