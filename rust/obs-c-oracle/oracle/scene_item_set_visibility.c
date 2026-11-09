/* Real original body and real production field adapters over complete private
 * layouts. The two active-child test doubles record calls, never abort. */
#include <string.h>
#include "util/threading.h"
#include "obs-scene.h"

static struct obs_scene_item *visibility_probe_item;
static unsigned visibility_probe_trace;
static unsigned visibility_probe_error;
static bool visibility_probe_add(obs_source_t *parent, obs_source_t *child)
{
	struct obs_scene_item *item = visibility_probe_item;
	visibility_probe_trace = visibility_probe_trace * 4 + 1;
	if (parent != item->parent->source || child != item->source || item->audio_actions.num != 0)
		visibility_probe_error |= 1;
	if (pthread_mutex_trylock(&item->actions_mutex) == 0) {
		visibility_probe_error |= 2;
		pthread_mutex_unlock(&item->actions_mutex);
	}
	return false; /* Deliberately ignored by both implementations. */
}
static void visibility_probe_remove(obs_source_t *parent, obs_source_t *child)
{
	visibility_probe_add(parent, child);
	visibility_probe_trace++;
}
#define obs_source_add_active_child visibility_probe_add
#define obs_source_remove_active_child visibility_probe_remove
#include "obs-scene-item-set-visibility.c.h"
#include "scene_item_set_visibility_fields.inc"
#undef obs_source_add_active_child
#undef obs_source_remove_active_child

typedef void (*visibility_reconcile)(void *, bool);
unsigned oracle_scene_item_set_visibility_probe(long refs, unsigned bits, unsigned queued,
						visibility_reconcile reconcile);
unsigned oracle_scene_item_set_visibility_probe(long refs, unsigned bits, unsigned queued,
						visibility_reconcile reconcile)
{
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	scene.source = (obs_source_t *)&scene;
	item.parent = &scene;
	item.source = (obs_source_t *)&item;
	pthread_mutex_init(&item.actions_mutex, NULL);
	da_resize(item.audio_actions, queued % 9);
	size_t capacity = item.audio_actions.capacity;
	void *buffer = item.audio_actions.array;
	unsigned traces[2];
	unsigned errors = 0;
	visibility_probe_item = &item;
	bool requested = (bits & 4) != 0;
	for (unsigned run = 0; run < 2; run++) {
		item.audio_actions.num = queued % 9;
		os_atomic_set_long(&item.active_refs, refs);
		os_atomic_set_bool(&item.visible, (bits & 1) != 0);
		os_atomic_set_bool(&item.user_visible, (bits & 2) != 0);
		item.selected = true;
		item.id = 173;
		visibility_probe_trace = 0;
		visibility_probe_error = 0;
		if (run == 0)
			set_visibility(&item, requested);
		else
			reconcile(&item, requested);
		traces[run] = visibility_probe_trace;
		errors |= visibility_probe_error;
		if (item.active_refs != (requested ? 1 : 0) || item.visible != requested ||
		    item.user_visible != requested || item.audio_actions.num != 0)
			errors |= 4;
		if (item.audio_actions.capacity != capacity || item.audio_actions.array != buffer ||
		    item.parent != &scene || item.source != (obs_source_t *)&item || !item.selected || item.id != 173)
			errors |= 8;
		if (pthread_mutex_trylock(&item.actions_mutex) != 0)
			errors |= 16;
		else
			pthread_mutex_unlock(&item.actions_mutex);
	}
	if (traces[0] != traces[1])
		errors |= 32;
	da_free(item.audio_actions);
	pthread_mutex_destroy(&item.actions_mutex);
	visibility_probe_item = NULL;
	return errors;
}
