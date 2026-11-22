#include "libobs/build/libobs.pch.h"

void obs_enter_graphics(void)
{
	if (obs->video.graphics)
		gs_enter_context(obs->video.graphics);
}

void obs_leave_graphics(void)
{
	if (obs->video.graphics)
		gs_leave_context();
}
