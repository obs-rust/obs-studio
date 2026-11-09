/* Original C getter and real atomic adapter; production is called only by callback. */
#include <stddef.h>
#include <string.h>
#include "util/threading.h"
#include "obs-scene.h"
bool oracle_obs_sceneitem_visible(const obs_sceneitem_t *item);
#define obs_sceneitem_visible oracle_obs_sceneitem_visible
#include "obs-scene-item-visible.c.h"
#undef obs_sceneitem_visible
#include "scene_item_visible_fields.inc"

typedef bool (*visible_getter)(const obs_sceneitem_t *);
unsigned oracle_scene_item_visible_probe(unsigned bits, visible_getter getter);

unsigned oracle_scene_item_visible_probe(unsigned bits, visible_getter getter)
{
	struct obs_scene_item item = {0};
	item.selected = (bits & 1) != 0;
	item.locked = (bits & 2) != 0;
	os_atomic_store_bool(&item.visible, (bits & 4) != 0);
	os_atomic_store_bool(&item.user_visible, (bits & 8) != 0);
	item.removed = (bits & 16) != 0;
	item.id = -17;
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	unsigned result = 0;
	if (getter(NULL) != oracle_obs_sceneitem_visible(NULL))
		result |= 1;
	if (getter(&item) != oracle_obs_sceneitem_visible(&item))
		result |= 2;
	if (memcmp(before, &item, sizeof(item)))
		result |= 4;
	/* A second call must reload user_visible after an atomic store. */
	os_atomic_store_bool(&item.user_visible, (bits & 8) == 0);
	memcpy(before, &item, sizeof(item));
	if (getter(&item) != oracle_obs_sceneitem_visible(&item))
		result |= 8;
	if (memcmp(before, &item, sizeof(item)))
		result |= 16;
	return result;
}
