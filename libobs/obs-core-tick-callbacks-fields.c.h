#include "libobs/build/libobs.pch.h"

struct darray *obs_core_tick_callbacks_array(struct obs_core *core)
{
	return &core->data.tick_callbacks.da;
}

void obs_core_tick_callbacks_lock(struct obs_core *core)
{
	pthread_mutex_lock(&core->data.draw_callbacks_mutex);
}

void obs_core_tick_callbacks_unlock(struct obs_core *core)
{
	pthread_mutex_unlock(&core->data.draw_callbacks_mutex);
}
