#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only original bodies over the C-owned TLS context getter. */
#define thread_graphics gs_get_context()
static inline bool samplerstate_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}
	return true;
}
static inline bool samplerstate_ref_obj_valid(const void *obj, const char *f, const char *name)
{
	if (!obj) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", f, name);
		return false;
	}
	return true;
}
#define gs_valid samplerstate_ref_valid
#define gs_obj_valid samplerstate_ref_obj_valid
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
#undef gs_obj_valid
#undef gs_valid
#undef thread_graphics
