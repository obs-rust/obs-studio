#include "studio-mode-exit.h"

#include <stddef.h>

/* OBSBasic::TransitionToScene(scene, true) outside Studio Mode. */
static void cut_to_scene(const struct studio_mode_exit_ops *ops, void *scene)
{
	ops->show_on_program(ops->param, scene);
	ops->scene_changed(ops->param);
}

void studio_mode_exit_switch(const struct studio_mode_exit_ops *ops, void *program_scene, void *current_scene)
{
	if (!program_scene) {
		cut_to_scene(ops, current_scene);
		return;
	}

	/* OBSBasic::SetCurrentScene(program, true) outside Studio Mode, then
	 * OBSBasic::TransitionToScene(program, true). */
	cut_to_scene(ops, program_scene);
	ops->select_scene(ops->param, program_scene);
	cut_to_scene(ops, program_scene);
}
