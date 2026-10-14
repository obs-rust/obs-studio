#pragma once

#include <obs.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Sources context menu > Show Transition / Hide Transition, without Qt.
 *
 * visibility_transition_set() applies a transition picked in the menu to
 * `item`, the item the menu is for, and to the other selected items of
 * `scene`, including the selected items inside its groups. An item that
 * already has a transition of that type keeps it (and its settings);
 * otherwise it gets a new private transition named "<source name>
 * <name_suffix>", and `default_duration` when it has no duration yet. An empty
 * or NULL `id` removes the transition and its duration.
 *
 * For undo, undo and redo (when not NULL) receive an "items" array with one
 * entry per changed item: the UUID of its scene or group ("scene"), its "id",
 * its source's "name" and its transition ("transition", as from
 * obs_sceneitem_transition_save()) before and after the change. Returns the
 * number of items changed.
 *
 * visibility_transition_restore() puts back the transitions saved in undo or
 * redo. visibility_transition_set_duration() sets the transition duration of
 * the same items as visibility_transition_set(). */
struct visibility_transition_change {
	bool show; /* the show transition (true) or the hide transition */
	const char *id;
	const char *name_suffix;
	uint32_t default_duration;
};

size_t visibility_transition_set(obs_scene_t *scene, obs_sceneitem_t *item,
				 const struct visibility_transition_change *change, obs_data_t *undo, obs_data_t *redo);
void visibility_transition_restore(obs_data_t *data);
void visibility_transition_set_duration(obs_scene_t *scene, obs_sceneitem_t *item, bool show, uint32_t duration);

#ifdef __cplusplus
}
#endif
