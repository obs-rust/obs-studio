#include "libobs/build/libobs.pch.h"

/* Private context, diagnostic and callback access only. */
graphics_t *obs_timer_range_context(void);
graphics_t *obs_timer_range_context(void)
{
	return gs_get_context();
}
void obs_timer_range_debug(const char *function, const char *reason);
void obs_timer_range_debug(const char *function, const char *reason)
{
	blog(LOG_DEBUG, "%s: %s", function, reason);
}
void (*obs_timer_range_destroy_field(graphics_t *graphics))(gs_timer_range_t *);
void (*obs_timer_range_destroy_field(graphics_t *graphics))(gs_timer_range_t *)
{
	return graphics->exports.gs_timer_range_destroy;
}
bool (*obs_timer_range_begin_field(graphics_t *graphics))(gs_timer_range_t *);
bool (*obs_timer_range_begin_field(graphics_t *graphics))(gs_timer_range_t *)
{
	return graphics->exports.gs_timer_range_begin;
}
bool (*obs_timer_range_end_field(graphics_t *graphics))(gs_timer_range_t *);
bool (*obs_timer_range_end_field(graphics_t *graphics))(gs_timer_range_t *)
{
	return graphics->exports.gs_timer_range_end;
}
bool (*obs_timer_range_data_field(graphics_t *graphics))(gs_timer_range_t *, bool *, uint64_t *);
bool (*obs_timer_range_data_field(graphics_t *graphics))(gs_timer_range_t *, bool *, uint64_t *)
{
	return graphics->exports.gs_timer_range_get_data;
}
