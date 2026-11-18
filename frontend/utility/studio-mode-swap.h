#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* "Swap Preview/Program Scenes After Transitioning" in Studio Mode, kept free
 * of Qt so it can be unit tested. When a transition to the program starts,
 * the frontend remembers the scene that was on the program (the swap scene)
 * and the scene in the preview. When the transition ends, the swap scene may
 * go to the preview. Scenes are opaque to the helper. */

/* swap_scene: the program scene from before the transition, or NULL if no
 * swap was scheduled or that scene is gone. preview_at_start: the scene in
 * the preview when the transition started, or NULL if it is gone.
 * preview_now: the scene in the preview now. Returns the scene to put in the
 * preview, or NULL to leave the preview as it is. */
void *studio_mode_swap_scene(void *swap_scene, void *preview_at_start, void *preview_now);

/* The Studio Mode scenes OBSBasic stores for the program: the program scene,
 * the program scene before it, and the swap scene and preview remembered for
 * studio_mode_swap_scene(). Each may be NULL (unset, or the scene is gone). */
struct studio_mode_program {
	void *program;
	void *last_program;
	void *swap_scene;
	void *swap_preview;
};

enum studio_mode_program_step {
	/* Go on with the switch. */
	STUDIO_MODE_PROGRAM_SWITCH,
	/* The target is already the program: nothing to start. */
	STUDIO_MODE_PROGRAM_SAME,
	/* A transition is still running: the switch is refused. */
	STUDIO_MODE_PROGRAM_REFUSED,
};

/* OBSBasic::TransitionToScene() in Studio Mode: a request to put target on
 * the program. preview_now is the scene in the preview. tbar_active: the
 * T-bar is held, so the program before it stays the last program.
 * check_same: neither forced nor a fade to or from black, so a request for
 * the program scene does nothing and a swap after the transition may be
 * scheduled. duplicate: "Duplicate Scene" is on. swap_mode: "Swap
 * Preview/Program Scenes After Transitioning" is on. still_transitioning: the
 * program transition is between its start and its end. Says how the
 * frontend goes on and updates *state, except for a refused switch, which
 * leaves it as it was. */
enum studio_mode_program_step studio_mode_program_switch(struct studio_mode_program *state, void *target,
							 void *preview_now, bool tbar_active, bool check_same,
							 bool duplicate, bool swap_mode, bool still_transitioning);

#ifdef __cplusplus
}
#endif
