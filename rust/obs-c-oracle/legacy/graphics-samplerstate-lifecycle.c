#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);
static inline bool gs_obj_valid(const void *obj, const char *f, const char *name);

#define gs_valid_p(func, param1) (gs_valid(func) && gs_obj_valid(param1, func, #param1))

gs_samplerstate_t *gs_samplerstate_create(const struct gs_sampler_info *info)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_samplerstate_create", info))
		return NULL;

	return graphics->exports.device_samplerstate_create(graphics->device, info);
}

void gs_samplerstate_destroy(gs_samplerstate_t *samplerstate)
{
	if (!gs_valid("gs_samplerstate_destroy"))
		return;
	if (!samplerstate)
		return;

	thread_graphics->exports.gs_samplerstate_destroy(samplerstate);
}

#undef gs_valid_p
