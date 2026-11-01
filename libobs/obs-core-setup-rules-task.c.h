/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs-core-setup-rules-task.pch.h"

/* The task queue's route and the video level checks, moved from
 * obs_queue_task(), obs_in_task_thread() and obs_set_video_levels() in
 * obs.c. Pure: no libobs state. obs.c keeps every read (the task handler,
 * the thread flags, os_task_queue_inside(), the core), every call (the
 * handler, the task, the wait event, the queue pushes and their locks,
 * os_task_queue_queue_task()), the assert and the logging, and passes the
 * values in, one step at a time. Declared in obs-core-setup-rules.h,
 * beside the other setup rules of obs-core-setup-rules.c. */

enum obs_core_setup_rules_task obs_core_setup_rules_task_step(enum obs_core_setup_rules_task step, int type, bool value)
{
	switch (step) {
	case OBS_CORE_SETUP_RULES_TASK_TYPE:
		return type == OBS_CORE_SETUP_RULES_TASK_UI ? OBS_CORE_SETUP_RULES_TASK_UI_HANDLER
							    : OBS_CORE_SETUP_RULES_TASK_IN_THREAD;
	case OBS_CORE_SETUP_RULES_TASK_UI_HANDLER:
		return value ? OBS_CORE_SETUP_RULES_TASK_UI_CALL : OBS_CORE_SETUP_RULES_TASK_UI_NONE;
	case OBS_CORE_SETUP_RULES_TASK_IN_THREAD:
		return value ? OBS_CORE_SETUP_RULES_TASK_RUN : OBS_CORE_SETUP_RULES_TASK_WAIT;
	case OBS_CORE_SETUP_RULES_TASK_WAIT:
		if (value)
			return OBS_CORE_SETUP_RULES_TASK_WAIT_QUEUE;
		else if (type == OBS_CORE_SETUP_RULES_TASK_GRAPHICS)
			return OBS_CORE_SETUP_RULES_TASK_PUSH_GRAPHICS;
		else if (type == OBS_CORE_SETUP_RULES_TASK_AUDIO)
			return OBS_CORE_SETUP_RULES_TASK_PUSH_AUDIO;
		else if (type == OBS_CORE_SETUP_RULES_TASK_DESTROY)
			return OBS_CORE_SETUP_RULES_TASK_PUSH_DESTROY;
		return OBS_CORE_SETUP_RULES_TASK_DROP;
	case OBS_CORE_SETUP_RULES_TASK_UI_CALL:
	case OBS_CORE_SETUP_RULES_TASK_UI_NONE:
	case OBS_CORE_SETUP_RULES_TASK_RUN:
	case OBS_CORE_SETUP_RULES_TASK_WAIT_QUEUE:
	case OBS_CORE_SETUP_RULES_TASK_PUSH_GRAPHICS:
	case OBS_CORE_SETUP_RULES_TASK_PUSH_AUDIO:
	case OBS_CORE_SETUP_RULES_TASK_PUSH_DESTROY:
	case OBS_CORE_SETUP_RULES_TASK_DROP:
	case OBS_CORE_SETUP_RULES_TASK_UNKNOWN:
		break;
	}
	return step;
}

int obs_core_setup_rules_task_thread(int type)
{
	if (type == OBS_CORE_SETUP_RULES_TASK_GRAPHICS)
		return OBS_CORE_SETUP_RULES_TASK_GRAPHICS;
	else if (type == OBS_CORE_SETUP_RULES_TASK_AUDIO)
		return OBS_CORE_SETUP_RULES_TASK_AUDIO;
	else if (type == OBS_CORE_SETUP_RULES_TASK_UI)
		return OBS_CORE_SETUP_RULES_TASK_UI;
	else if (type == OBS_CORE_SETUP_RULES_TASK_DESTROY)
		return OBS_CORE_SETUP_RULES_TASK_DESTROY;
	return -1;
}

/* obs_set_video_levels() reads obs->video with or without a core. */
bool obs_core_setup_rules_levels_core(bool core)
{
	(void)core;
	return true;
}

/* The draw paths divide by these, so a level that is not finite and
 * greater than 0 is ignored and the previous one stays (#2209). */
bool obs_core_setup_rules_level_ok(float level)
{
	return isfinite(level) && level > 0.f;
}
