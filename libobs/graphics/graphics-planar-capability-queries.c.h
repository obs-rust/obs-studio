#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);

bool gs_nv12_available(void)
{
	if (!gs_valid("gs_nv12_available"))
		return false;

	if (!thread_graphics->exports.device_nv12_available)
		return false;

	return thread_graphics->exports.device_nv12_available(thread_graphics->device);
}

bool gs_p010_available(void)
{
	if (!gs_valid("gs_p010_available"))
		return false;

	if (!thread_graphics->exports.device_p010_available)
		return false;

	return thread_graphics->exports.device_p010_available(thread_graphics->device);
}
