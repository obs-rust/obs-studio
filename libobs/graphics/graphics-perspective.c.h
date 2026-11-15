#include "libobs/build/libobs-graphics.pch.h"

/* Keep graphics.c.h's context/validation seams, as vertexbuffer-create.c.h does. */
#define thread_graphics gs_get_context()
static inline bool perspective_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}
#define gs_valid perspective_valid

/* Preserve the near/far parameter names when the Windows SDK defines them. */
#pragma push_macro("near")
#pragma push_macro("far")
#ifdef near
#undef near
#endif
#ifdef far
#undef far
#endif

void gs_perspective(float angle, float aspect, float near, float far)
{
	graphics_t *graphics = thread_graphics;
	float xmin, xmax, ymin, ymax;

	if (!gs_valid("gs_perspective"))
		return;

	ymax = near * tanf(RAD(angle) * 0.5f);
	ymin = -ymax;

	xmin = ymin * aspect;
	xmax = ymax * aspect;

	graphics->exports.device_frustum(graphics->device, xmin, xmax, ymin, ymax, near, far);
}

#pragma pop_macro("far")
#pragma pop_macro("near")
#undef gs_valid
#undef thread_graphics
