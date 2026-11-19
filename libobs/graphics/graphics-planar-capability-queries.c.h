#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only: read the existing TLS context without owning another one. */
#define thread_graphics gs_get_context()

static inline bool planar_capability_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

#define gs_valid planar_capability_ref_valid

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

#undef gs_valid
#undef thread_graphics
