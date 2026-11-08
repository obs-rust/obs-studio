/* Real-header fixtures and the unchanged original borrowed-parent query. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"
obs_scene_t *oracle_obs_sceneitem_get_scene(const obs_sceneitem_t *item);
#define obs_sceneitem_get_scene oracle_obs_sceneitem_get_scene
#include "obs-scene-item-parent-getter.c.h"
#undef obs_sceneitem_get_scene
#include "scene_item_parent_fields.inc"

struct scene_parent_fixture {
	struct obs_scene_item item;
	struct obs_scene scenes[2];
	unsigned char item_before[sizeof(struct obs_scene_item)];
	unsigned char scenes_before[2 * sizeof(struct obs_scene)];
};

void oracle_scene_parent_layout(size_t *layout);
void oracle_scene_parent_layout(size_t *layout)
{
	layout[0] = sizeof(((struct obs_scene_item *)0)->parent);
	layout[1] = _Alignof(obs_scene_t *);
}

void *oracle_scene_parent_new(unsigned bits);
void *oracle_scene_parent_new(unsigned bits)
{
	struct scene_parent_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->item.is_group = (bits & 1) != 0;
	fixture->item.removed = (bits & 2) != 0;
	fixture->item.id = -17;
	fixture->item.parent = (bits & 4) ? &fixture->scenes[(bits >> 3) & 1] : NULL;
	memcpy(fixture->item_before, &fixture->item, sizeof(fixture->item));
	memcpy(fixture->scenes_before, fixture->scenes, sizeof(fixture->scenes));
	return fixture;
}

bool oracle_scene_parent_unchanged(const void *handle);
bool oracle_scene_parent_unchanged(const void *handle)
{
	const struct scene_parent_fixture *fixture = handle;
	return memcmp(fixture->item_before, &fixture->item, sizeof(fixture->item)) == 0 &&
	       memcmp(fixture->scenes_before, fixture->scenes, sizeof(fixture->scenes)) == 0;
}

void oracle_scene_parent_free(void *handle);
void oracle_scene_parent_free(void *handle)
{
	free(handle);
}
