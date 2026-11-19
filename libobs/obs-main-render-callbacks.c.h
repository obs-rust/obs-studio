#include "libobs/build/libobs.pch.h"

void obs_add_main_render_callback(void (*draw)(void *param, uint32_t cx, uint32_t cy), void *param)
{
	struct draw_callback data = {draw, param};

	pthread_mutex_lock(&obs->data.draw_callbacks_mutex);
	da_insert(obs->data.draw_callbacks, 0, &data);
	pthread_mutex_unlock(&obs->data.draw_callbacks_mutex);
}

void obs_remove_main_render_callback(void (*draw)(void *param, uint32_t cx, uint32_t cy), void *param)
{
	struct draw_callback data = {draw, param};

	pthread_mutex_lock(&obs->data.draw_callbacks_mutex);
	da_erase_item(obs->data.draw_callbacks, &data);
	pthread_mutex_unlock(&obs->data.draw_callbacks_mutex);
}
