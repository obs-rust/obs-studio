/* Original getters and the automatically discovered private field adapter. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_sceneitem_get_transition oracle_obs_sceneitem_get_transition
#define obs_sceneitem_get_transition_duration oracle_obs_sceneitem_get_transition_duration
#include "obs-scene-item-transition-getters.c.h"
#undef obs_sceneitem_get_transition
#undef obs_sceneitem_get_transition_duration
#include "scene_item_transition_fields.inc"

struct scene_item_transition_fixture {
	struct obs_scene_item item;
	struct obs_scene_item before;
	struct obs_source shown;
	struct obs_source hidden;
};

void oracle_scene_item_transition_layout(size_t *layout);
void oracle_scene_item_transition_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_scene_item_transition_fields);
	layout[1] = _Alignof(struct obs_scene_item_transition_fields);
	layout[2] = offsetof(struct obs_scene_item_transition_fields, show_transition);
	layout[3] = offsetof(struct obs_scene_item_transition_fields, hide_transition);
	layout[4] = offsetof(struct obs_scene_item_transition_fields, show_duration);
	layout[5] = offsetof(struct obs_scene_item_transition_fields, hide_duration);
	layout[6] = sizeof(uint32_t);
	layout[7] = _Alignof(uint32_t);
}

void *oracle_scene_item_transition_new(uint32_t shown, uint32_t hidden, bool has_show, bool has_hide);
void *oracle_scene_item_transition_new(uint32_t shown, uint32_t hidden, bool has_show, bool has_hide)
{
	struct scene_item_transition_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->item.show_transition = has_show ? &fixture->shown : NULL;
	fixture->item.hide_transition = has_hide ? &fixture->hidden : NULL;
	fixture->item.show_transition_duration = shown;
	fixture->item.hide_transition_duration = hidden;
	fixture->item.id = 901;
	memcpy(&fixture->before, &fixture->item, sizeof(fixture->item));
	return fixture;
}

bool oracle_scene_item_transition_unchanged(const void *handle);
bool oracle_scene_item_transition_unchanged(const void *handle)
{
	const struct scene_item_transition_fixture *fixture = handle;
	return memcmp(&fixture->item, &fixture->before, sizeof(fixture->item)) == 0;
}

void oracle_scene_item_transition_free(void *handle);
void oracle_scene_item_transition_free(void *handle)
{
	free(handle);
}
