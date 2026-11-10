#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only equivalents of the graphics unity group private validators. */
static bool shader_file_valid(const char *name)
{
	if (!gs_get_context()) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", name);
		return false;
	}
	return true;
}

static bool shader_file_obj_valid(const void *ptr, const char *name, const char *parameter)
{
	if (!ptr) {
		blog(LOG_DEBUG, "%s: Null '%s' parameter", name, parameter);
		return false;
	}
	return true;
}
