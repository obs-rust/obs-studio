#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);

int gs_get_device_type(void)
{
	return gs_valid("gs_get_device_type") ? thread_graphics->exports.device_get_type() : -1;
}

uint32_t gs_get_adapter_count(void)
{
	if (!gs_valid("gs_get_adapter_count"))
		return 0;
	if (!thread_graphics->exports.gs_get_adapter_count)
		return 0;

	return thread_graphics->exports.gs_get_adapter_count();
}
