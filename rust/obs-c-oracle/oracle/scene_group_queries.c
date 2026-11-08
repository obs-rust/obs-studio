/* Real-header fixtures and the unchanged original group queries. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"
bool oracle_obs_sceneitem_is_group(obs_sceneitem_t *item);
bool oracle_obs_scene_is_group(const obs_scene_t *scene);
#define obs_sceneitem_is_group oracle_obs_sceneitem_is_group
#define obs_scene_is_group oracle_obs_scene_is_group
#include "obs-scene-group-queries.c.h"
#undef obs_sceneitem_is_group
#undef obs_scene_is_group
#include "scene_group_query_fields.inc"

struct scene_group_fixture {
	struct obs_scene scene;
	struct obs_scene_item item;
	unsigned char scene_before[sizeof(struct obs_scene)];
	unsigned char item_before[sizeof(struct obs_scene_item)];
};

void oracle_scene_group_layout(size_t *layout);
void oracle_scene_group_layout(size_t *layout)
{
	layout[0] = sizeof(((struct obs_scene *)0)->is_group);
	layout[1] = sizeof(((struct obs_scene_item *)0)->is_group);
	layout[2] = _Alignof(bool);
}

void *oracle_scene_group_new(unsigned bits);
void *oracle_scene_group_new(unsigned bits)
{
	struct scene_group_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->item.is_group = (bits & 1) != 0;
	fixture->scene.is_group = (bits & 2) != 0;
	fixture->item.is_scene = (bits & 4) != 0;
	fixture->item.removed = (bits & 8) != 0;
	fixture->item.id = -17;
	memcpy(fixture->scene_before, &fixture->scene, sizeof(fixture->scene));
	memcpy(fixture->item_before, &fixture->item, sizeof(fixture->item));
	return fixture;
}

void *oracle_scene_group_item(void *handle);
void *oracle_scene_group_item(void *handle)
{
	struct scene_group_fixture *fixture = handle;
	return &fixture->item;
}

bool oracle_scene_group_unchanged(const void *handle);
bool oracle_scene_group_unchanged(const void *handle)
{
	const struct scene_group_fixture *fixture = handle;
	return memcmp(fixture->scene_before, &fixture->scene, sizeof(fixture->scene)) == 0 &&
	       memcmp(fixture->item_before, &fixture->item, sizeof(fixture->item)) == 0;
}

void oracle_scene_group_free(void *handle);
void oracle_scene_group_free(void *handle)
{
	free(handle);
}
