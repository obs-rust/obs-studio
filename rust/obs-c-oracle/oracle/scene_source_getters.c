/* Original borrowed queries and automatically discovered real-layout adapters. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"
obs_source_t *oracle_obs_scene_get_source(const obs_scene_t *scene);
obs_source_t *oracle_obs_sceneitem_get_source(const obs_sceneitem_t *item);
#define obs_scene_get_source oracle_obs_scene_get_source
#define obs_sceneitem_get_source oracle_obs_sceneitem_get_source
#include "obs-scene-source-getters.c.h"
#undef obs_scene_get_source
#undef obs_sceneitem_get_source
#include "scene_source_fields.inc"

struct scene_source_fixture {
	struct obs_scene scene;
	struct obs_scene_item item;
	struct obs_source sources[2];
	unsigned char scene_before[sizeof(struct obs_scene)];
	unsigned char item_before[sizeof(struct obs_scene_item)];
	unsigned char sources_before[2 * sizeof(struct obs_source)];
};

void oracle_scene_source_layout(size_t *layout);
void oracle_scene_source_layout(size_t *layout)
{
	layout[0] = sizeof(((struct obs_scene *)0)->source);
	layout[1] = _Alignof(obs_source_t *);
	layout[2] = sizeof(((struct obs_scene_item *)0)->source);
	layout[3] = _Alignof(obs_source_t *);
}

void *oracle_scene_source_new(unsigned bits);
void *oracle_scene_source_new(unsigned bits)
{
	struct scene_source_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->scene.source = (bits & 1) ? &fixture->sources[(bits >> 2) & 1] : NULL;
	fixture->scene.first_item = &fixture->item;
	fixture->item.source = (bits & 2) ? &fixture->sources[(bits >> 3) & 1] : NULL;
	fixture->item.parent = &fixture->scene;
	fixture->item.id = -17;
	fixture->sources[0].context.name = "scene source sentinel";
	fixture->sources[1].context.name = "item source sentinel";
	memcpy(fixture->scene_before, &fixture->scene, sizeof(fixture->scene));
	memcpy(fixture->item_before, &fixture->item, sizeof(fixture->item));
	memcpy(fixture->sources_before, fixture->sources, sizeof(fixture->sources));
	return fixture;
}

void *oracle_scene_source_item(void *handle);
void *oracle_scene_source_item(void *handle)
{
	return &((struct scene_source_fixture *)handle)->item;
}

bool oracle_scene_source_unchanged(const void *handle);
bool oracle_scene_source_unchanged(const void *handle)
{
	const struct scene_source_fixture *fixture = handle;
	return memcmp(fixture->scene_before, &fixture->scene, sizeof(fixture->scene)) == 0 &&
	       memcmp(fixture->item_before, &fixture->item, sizeof(fixture->item)) == 0 &&
	       memcmp(fixture->sources_before, fixture->sources, sizeof(fixture->sources)) == 0;
}

void oracle_scene_source_free(void *handle);
void oracle_scene_source_free(void *handle)
{
	free(handle);
}
