#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only context validator; the original body remains in the oracle. */
#define thread_graphics gs_get_context()
static inline bool device_identity_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}
	return true;
}
#define gs_valid device_identity_ref_valid

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

#undef gs_valid
#undef thread_graphics
