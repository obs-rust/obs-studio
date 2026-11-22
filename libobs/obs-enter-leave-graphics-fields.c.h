#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_enter_graphics() and obs_leave_graphics():
 * the core's graphics subsystem and the two graphics-layer calls. The core
 * layout stays in C; Rust decides when each is called. */
graphics_t *obs_enter_leave_graphics_get(void);
graphics_t *obs_enter_leave_graphics_get(void)
{
	return obs->video.graphics;
}

void obs_enter_leave_graphics_enter(graphics_t *graphics);
void obs_enter_leave_graphics_enter(graphics_t *graphics)
{
	gs_enter_context(graphics);
}

void obs_enter_leave_graphics_leave(void);
void obs_enter_leave_graphics_leave(void)
{
	gs_leave_context();
}
