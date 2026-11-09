#include "libobs/build/libobs-graphics.pch.h"

bool obs_graphics_viewport_valid(const char *function);
bool obs_graphics_viewport_maximum(graphics_t *graphics, uint32_t *value);
int obs_graphics_viewport_type(graphics_t *graphics);

bool obs_graphics_viewport_valid(const char *function)
{
	if (!gs_get_context()) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
		return false;
	}
	return true;
}
bool obs_graphics_viewport_maximum(graphics_t *graphics, uint32_t *value)
{
	if (!graphics->exports.device_get_max_texture_size)
		return false;
	*value = graphics->exports.device_get_max_texture_size(graphics->device);
	return true;
}
int obs_graphics_viewport_type(graphics_t *graphics)
{
	return graphics->exports.device_get_type();
}

void obs_graphics_viewport_size(uint32_t *width, uint32_t *height);
void obs_graphics_viewport_set(int x, int y, int width, int height);
void obs_graphics_viewport_ortho(float left, float right, float top, float bottom, float znear, float zfar);
void obs_graphics_viewport_size(uint32_t *width, uint32_t *height)
{
	gs_get_size(width, height);
}
void obs_graphics_viewport_set(int x, int y, int width, int height)
{
	gs_set_viewport(x, y, width, height);
}
void obs_graphics_viewport_ortho(float left, float right, float top, float bottom, float znear, float zfar)
{
	gs_ortho(left, right, top, bottom, znear, zfar);
}
