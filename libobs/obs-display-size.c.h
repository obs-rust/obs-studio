#include "libobs/build/libobs-ported-view.pch.h"

void obs_display_size(obs_display_t *display, uint32_t *width, uint32_t *height)
{
	*width = 0;
	*height = 0;

	if (display) {
		pthread_mutex_lock(&display->draw_info_mutex);

		*width = display->cx;
		*height = display->cy;

		pthread_mutex_unlock(&display->draw_info_mutex);
	}
}
