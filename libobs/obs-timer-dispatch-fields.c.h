#include "libobs/build/libobs.pch.h"

/* Private context, diagnostic and callback access only. */
graphics_t *obs_timer_context(void);
graphics_t *obs_timer_context(void)
{
	return gs_get_context();
}
void obs_timer_debug(const char *function, const char *reason);
void obs_timer_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}
void (*obs_timer_destroy_field(graphics_t *graphics))(gs_timer_t *);
void (*obs_timer_destroy_field(graphics_t *graphics))(gs_timer_t *)
{
	return graphics->exports.gs_timer_destroy;
}
void (*obs_timer_begin_field(graphics_t *graphics))(gs_timer_t *);
void (*obs_timer_begin_field(graphics_t *graphics))(gs_timer_t *)
{
	return graphics->exports.gs_timer_begin;
}
void (*obs_timer_end_field(graphics_t *graphics))(gs_timer_t *);
void (*obs_timer_end_field(graphics_t *graphics))(gs_timer_t *)
{
	return graphics->exports.gs_timer_end;
}
bool (*obs_timer_data_field(graphics_t *graphics))(gs_timer_t *, uint64_t *);
bool (*obs_timer_data_field(graphics_t *graphics))(gs_timer_t *, uint64_t *)
{
	return graphics->exports.gs_timer_get_data;
}
