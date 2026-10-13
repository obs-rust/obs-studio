#include "studio-mode-exit.h"

#include <stddef.h>

void studio_mode_exit_switch(const struct studio_mode_exit_ops *ops, void *program_scene, void *current_scene)
{
	if (!program_scene) {
		/* The program scene is gone: the current scene goes to the
		 * program, which is a scene change. */
		ops->show_on_program(ops->param, current_scene);
		ops->scene_changed(ops->param);
		return;
	}

	/* The program already shows this scene, so the current scene stays
	 * the same for OBS_FRONTEND_EVENT_SCENE_CHANGED listeners: no event.
	 * Selecting it in the scene list first keeps
	 * obs_frontend_get_current_scene() on it throughout. Putting it on the
	 * program again replaces the private copy Studio Mode shows with
	 * "Duplicate Scene" on. */
	ops->select_scene(ops->param, program_scene);
	ops->show_on_program(ops->param, program_scene);
}
