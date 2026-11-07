#include "libobs/build/libobs-ported-view.pch.h"

void obs_display_set_enabled(obs_display_t *display, bool enable)
{
	if (display)
		display->enabled = enable;
}

bool obs_display_enabled(obs_display_t *display)
{
	return display ? display->enabled : false;
}

void obs_display_set_background_color(obs_display_t *display, uint32_t color)
{
	if (display)
		display->background_color = color;
}
