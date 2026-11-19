#include "libobs/build/libobs.pch.h"

void obs_add_main_rendered_callback(void (*rendered)(void *param), void *param)
{
	struct rendered_callback data = {rendered, param};

	pthread_mutex_lock(&obs->data.draw_callbacks_mutex);
	da_insert(obs->data.rendered_callbacks, 0, &data);
	pthread_mutex_unlock(&obs->data.draw_callbacks_mutex);
}

void obs_remove_main_rendered_callback(void (*rendered)(void *param), void *param)
{
	struct rendered_callback data = {rendered, param};

	pthread_mutex_lock(&obs->data.draw_callbacks_mutex);
	da_erase_item(obs->data.rendered_callbacks, &data);
	pthread_mutex_unlock(&obs->data.draw_callbacks_mutex);
}
