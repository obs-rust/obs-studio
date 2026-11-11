#include "libobs/build/libobs.pch.h"

/* Private context and field access only; validation and dispatch live in Rust. */
graphics_t *obs_depth_context(void);
graphics_t *obs_depth_context(void)
{
	return gs_get_context();
}

gs_device_t *obs_depth_device(graphics_t *graphics);
gs_device_t *obs_depth_device(graphics_t *graphics)
{
	return graphics->device;
}

void obs_depth_debug(const char *function);
void obs_depth_debug(const char *function)
{
	blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
}

void (*obs_depth_enable_field(graphics_t *graphics))(gs_device_t *, bool);
void (*obs_depth_enable_field(graphics_t *graphics))(gs_device_t *, bool)
{
	return graphics->exports.device_enable_depth_test;
}

void (*obs_depth_function_field(graphics_t *graphics))(gs_device_t *, enum gs_depth_test);
void (*obs_depth_function_field(graphics_t *graphics))(gs_device_t *, enum gs_depth_test)
{
	return graphics->exports.device_depth_function;
}
