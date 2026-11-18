#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only group: preserve the original context validator and bodies
 * over the existing TLS context getter. */
#define thread_graphics gs_get_context()

static inline bool device_queries_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}

#define gs_valid device_queries_ref_valid

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

#undef gs_valid
#undef thread_graphics
