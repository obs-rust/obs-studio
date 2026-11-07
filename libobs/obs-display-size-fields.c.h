#include "libobs/build/libobs.pch.h"

struct obs_display_size_fields obs_display_size_fields(obs_display_t *display)
{
	return (struct obs_display_size_fields){&display->cx, &display->cy};
}

void obs_display_size_lock(obs_display_t *display)
{
	pthread_mutex_lock(&display->draw_info_mutex);
}

void obs_display_size_unlock(obs_display_t *display)
{
	pthread_mutex_unlock(&display->draw_info_mutex);
}
