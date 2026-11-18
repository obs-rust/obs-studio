#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);

void gs_get_size(uint32_t *x, uint32_t *y)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_size"))
		return;

	graphics->exports.device_get_size(graphics->device, x, y);
}

uint32_t gs_get_width(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_width"))
		return 0;

	return graphics->exports.device_get_width(graphics->device);
}

uint32_t gs_get_height(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_height"))
		return 0;

	return graphics->exports.device_get_height(graphics->device);
}
