#pragma once

#include "obs.h"

struct obs_scene_item_transition_fields {
	obs_source_t *const *show_transition;
	obs_source_t *const *hide_transition;
	const uint32_t *show_duration;
	const uint32_t *hide_duration;
};
extern struct obs_scene_item_transition_fields obs_scene_item_transition_fields(const obs_sceneitem_t *item);
