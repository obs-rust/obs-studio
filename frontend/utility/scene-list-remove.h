#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Removing a scene from the scene list, kept free of Qt so it can be unit
 * tested. OBS_FRONTEND_EVENT_SCENE_LIST_CHANGED listeners (obs-websocket's
 * SceneListChanged, ...) are told about the change. Scenes are opaque to the
 * helper. */

struct scene_list_remove_ops {
	void *param;

	/* Removes the scene from libobs (obs_source_remove) and releases the
	 * sources only it used. libobs signals "source_remove", which reaches
	 * scene_list_on_scene_removed() through OBSBasic::RemoveScene before
	 * this returns. */
	void (*remove_source)(void *param, void *scene);

	/* Takes the scene's item out of the scene list. Returns false if the
	 * scene was not listed. */
	bool (*take_from_list)(void *param, void *scene);

	/* Emits OBS_FRONTEND_EVENT_SCENE_LIST_CHANGED. */
	void (*scene_list_changed)(void *param);
};

/* libobs removed the scene (OBSBasic::RemoveScene, the handler for
 * "source_remove"): by the user, undo/redo, obs-websocket or a script. */
void scene_list_on_scene_removed(const struct scene_list_remove_ops *ops, void *scene);

/* The user removed the scene selected in the scene list
 * (OBSBasic::RemoveSelectedScene, after the confirmation). Announced once,
 * by scene_list_on_scene_removed(). */
void scene_list_remove_selected(const struct scene_list_remove_ops *ops, void *scene);

#ifdef __cplusplus
}
#endif
