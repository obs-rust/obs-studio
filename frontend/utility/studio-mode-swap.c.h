#include "frontend/build/frontend-utility-studio-mode-swap.pch.h"

void *studio_mode_swap_scene(void *swap_scene, void *preview_at_start, void *preview_now)
{
	/* A preview chosen while the transition ran (a click in the scene
	 * list, obs_frontend_set_current_preview_scene(), obs-websocket's
	 * SetCurrentPreviewScene) stays. */
	if (preview_now != preview_at_start) {
		return NULL;
	}

	return swap_scene;
}

enum studio_mode_program_step studio_mode_program_switch(struct studio_mode_program *state, void *target,
							 void *preview_now, bool tbar_active, bool check_same,
							 bool duplicate, bool swap_mode, bool still_transitioning)
{
	if (!tbar_active) {
		state->last_program = state->program;
	}
	state->program = target;

	if (check_same) {
		void *previous = state->last_program;

		if (!duplicate && previous == target) {
			return STUDIO_MODE_PROGRAM_SAME;
		}

		if (swap_mode && previous && previous != preview_now) {
			state->swap_scene = previous;
			state->swap_preview = preview_now;
		}
	}

	if (still_transitioning) {
		return STUDIO_MODE_PROGRAM_REFUSED;
	}

	return STUDIO_MODE_PROGRAM_SWITCH;
}
