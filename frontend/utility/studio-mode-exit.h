#pragma once

#ifdef __cplusplus
extern "C" {
#endif

/* The scene switch made when Studio Mode is turned off, kept free of Qt so it
 * can be unit tested. Once Studio Mode is off, the scene shown on the program
 * becomes the current scene: the one selected in the scene list, which is
 * what obs_frontend_get_current_scene() returns and what
 * OBS_FRONTEND_EVENT_SCENE_CHANGED listeners (obs-websocket's
 * CurrentProgramSceneChanged, Advanced Scene Switcher, ...) read. Scenes are
 * opaque to the helper. */

struct studio_mode_exit_ops {
	void *param;

	/* Makes the scene the current scene in the scene list. Does not touch
	 * the program output and emits no OBS_FRONTEND_EVENT_SCENE_CHANGED. */
	void (*select_scene)(void *param, void *scene);

	/* Puts the scene on the program output at once (no transition). Emits
	 * nothing. */
	void (*show_on_program)(void *param, void *scene);

	/* Emits OBS_FRONTEND_EVENT_SCENE_CHANGED. */
	void (*scene_changed)(void *param);
};

/* program_scene: the scene on the program when Studio Mode is turned off, or
 * NULL if it is gone. current_scene: the scene selected in the scene list
 * (Studio Mode's preview). */
void studio_mode_exit_switch(const struct studio_mode_exit_ops *ops, void *program_scene, void *current_scene);

#ifdef __cplusplus
}
#endif
