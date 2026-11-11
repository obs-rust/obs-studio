#include "libobs/build/libobs-graphics.pch.h"

/* Private context layout and variadic logging stay in C. These adapters never
 * retain a context or buffer beyond the calling wrapper. */
void *obs_vertexbuffer_access_context(void);
void obs_vertexbuffer_access_flush(void *context, void *buffer);
void *obs_vertexbuffer_access_get_data(void *context, const void *buffer);
void obs_vertexbuffer_access_error(const char *function, bool no_context);

void *obs_vertexbuffer_access_context(void)
{
	return gs_get_context();
}

void obs_vertexbuffer_access_flush(void *context, void *buffer)
{
	if (!context || !buffer)
		return;
	graphics_t *graphics = context;
	graphics->exports.gs_vertexbuffer_flush(buffer);
}

void *obs_vertexbuffer_access_get_data(void *context, const void *buffer)
{
	if (!context || !buffer)
		return NULL;
	graphics_t *graphics = context;
	return graphics->exports.gs_vertexbuffer_get_data(buffer);
}

void obs_vertexbuffer_access_error(const char *function, bool no_context)
{
	if (no_context)
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
	else
		blog(LOG_DEBUG, "%s: Null '%s' parameter", function, "vertbuffer");
}
