#pragma once

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

#ifdef __cplusplus
}
#endif
