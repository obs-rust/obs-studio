#include "studio-mode-swap.h"

#include <stddef.h>

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
