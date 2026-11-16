#include "libobs/build/libobs.pch.h"

void obs_add_tick_callback(void (*tick)(void *param, float seconds), void *param)
{
	struct tick_callback data = {tick, param};

	pthread_mutex_lock(&obs->data.draw_callbacks_mutex);
	da_insert(obs->data.tick_callbacks, 0, &data);
	pthread_mutex_unlock(&obs->data.draw_callbacks_mutex);
}

void obs_remove_tick_callback(void (*tick)(void *param, float seconds), void *param)
{
	struct tick_callback data = {tick, param};

	pthread_mutex_lock(&obs->data.draw_callbacks_mutex);
	da_erase_item(obs->data.tick_callbacks, &data);
	pthread_mutex_unlock(&obs->data.draw_callbacks_mutex);
}
