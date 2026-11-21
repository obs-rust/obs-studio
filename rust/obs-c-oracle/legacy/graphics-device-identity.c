#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);

void *gs_get_device_obj(void)
{
	if (!gs_valid("gs_get_device_obj"))
		return NULL;

	return thread_graphics->exports.device_get_device_obj(thread_graphics->device);
}

const char *gs_get_device_name(void)
{
	return gs_valid("gs_get_device_name") ? thread_graphics->exports.device_get_name() : NULL;
}
