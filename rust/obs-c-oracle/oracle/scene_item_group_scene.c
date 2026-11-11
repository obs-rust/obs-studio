/* Original C getter; the production implementation is reached only by callback. */
#include <stddef.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"
obs_scene_t *oracle_obs_sceneitem_group_get_scene(const obs_sceneitem_t *item);
#define obs_sceneitem_group_get_scene oracle_obs_sceneitem_group_get_scene
#include "obs-scene-item-group-scene.c.h"
#undef obs_sceneitem_group_get_scene
#include "scene_item_group_scene_fields.inc"

typedef obs_scene_t *(*group_scene_getter)(const obs_sceneitem_t *);
unsigned oracle_scene_item_group_scene_probe(unsigned bits, group_scene_getter getter);

unsigned oracle_scene_item_group_scene_probe(unsigned bits, group_scene_getter getter)
{
	struct obs_scene scenes[2] = {0};
	struct obs_source source = {0};
	struct obs_scene_item item = {0};
	item.is_group = (bits & 1) != 0;
	item.selected = (bits & 2) != 0;
	item.removed = (bits & 4) != 0;
	item.id = -17;
	source.context.data = (bits & 8) ? &scenes[(bits >> 4) & 1] : NULL;
	item.source = (item.is_group || (bits & 2)) ? &source : NULL;
	unsigned char item_before[sizeof(item)], source_before[sizeof(source)], scenes_before[sizeof(scenes)];
	memcpy(item_before, &item, sizeof(item));
	memcpy(source_before, &source, sizeof(source));
	memcpy(scenes_before, scenes, sizeof(scenes));
	unsigned result = 0;
	if (getter(NULL) != oracle_obs_sceneitem_group_get_scene(NULL))
		result |= 1;
	if (getter(&item) != oracle_obs_sceneitem_group_get_scene(&item))
		result |= 2;
	if (getter(&item) != oracle_obs_sceneitem_group_get_scene(&item))
		result |= 4;
	if (memcmp(item_before, &item, sizeof(item)) || memcmp(source_before, &source, sizeof(source)) ||
	    memcmp(scenes_before, scenes, sizeof(scenes)))
		result |= 8;
	/* Each call must observe a changed context pointer, without taking a ref. */
	source.context.data = &scenes[((bits >> 4) & 1) ^ 1];
	memcpy(source_before, &source, sizeof(source));
	if (getter(&item) != oracle_obs_sceneitem_group_get_scene(&item))
		result |= 16;
	if (memcmp(item_before, &item, sizeof(item)) || memcmp(source_before, &source, sizeof(source)) ||
	    memcmp(scenes_before, scenes, sizeof(scenes)))
		result |= 32;
	return result;
}
