/* Original registration, real private layouts, and the production adapter. */
#define obs tick_callbacks_oracle_core
#define obs_add_tick_callback oracle_obs_add_tick_callback
#define obs_remove_tick_callback oracle_obs_remove_tick_callback
#include "libobs/obs-core-tick-callbacks.c.h"
#undef obs_remove_tick_callback
#undef obs_add_tick_callback
#undef obs
#include "core_tick_callbacks_fields.inc"
#include <stddef.h>
#include <stdlib.h>

struct obs_core *tick_callbacks_oracle_core;
void oracle_tick_callbacks_select(void *core);
void *oracle_tick_callbacks_new(void);
void oracle_tick_callbacks_free(void *core);
size_t oracle_tick_callbacks_layout(size_t index);
size_t oracle_tick_callbacks_num(void *core);
struct tick_callback oracle_tick_callbacks_get(void *core, size_t index);
void oracle_tick_callbacks_dispatch(void *core, float seconds);

void oracle_tick_callbacks_select(void *core)
{
	tick_callbacks_oracle_core = core;
}

void *oracle_tick_callbacks_new(void)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	if (!core)
		return NULL;
	if (pthread_mutex_init_recursive(&core->data.draw_callbacks_mutex)) {
		free(core);
		return NULL;
	}
	return core;
}

void oracle_tick_callbacks_free(void *opaque)
{
	struct obs_core *core = opaque;
	da_free(core->data.tick_callbacks);
	pthread_mutex_destroy(&core->data.draw_callbacks_mutex);
	free(core);
}

size_t oracle_tick_callbacks_layout(size_t index)
{
	const size_t layout[] = {sizeof(struct tick_callback), _Alignof(struct tick_callback),
				 offsetof(struct tick_callback, tick), offsetof(struct tick_callback, param)};
	return index < sizeof(layout) / sizeof(layout[0]) ? layout[index] : SIZE_MAX;
}

size_t oracle_tick_callbacks_num(void *opaque)
{
	struct obs_core *core = opaque;
	return core->data.tick_callbacks.num;
}

struct tick_callback oracle_tick_callbacks_get(void *opaque, size_t index)
{
	struct obs_core *core = opaque;
	return core->data.tick_callbacks.array[index];
}

void oracle_tick_callbacks_dispatch(void *opaque, float seconds)
{
	struct obs_core *core = opaque;
	struct obs_core_data *data = &core->data;
	/* The unchanged tick_sources loop: reverse traversal under the draw lock. */
	pthread_mutex_lock(&data->draw_callbacks_mutex);
	for (size_t i = data->tick_callbacks.num; i > 0; i--) {
		struct tick_callback *callback = data->tick_callbacks.array + (i - 1);
		callback->tick(callback->param, seconds);
	}
	pthread_mutex_unlock(&data->draw_callbacks_mutex);
}
