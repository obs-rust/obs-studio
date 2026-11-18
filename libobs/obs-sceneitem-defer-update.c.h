#include "libobs/build/libobs.pch.h"

/* obs_sceneitem_defer_update_begin() and obs_sceneitem_defer_update_end(),
 * moved verbatim out of obs-scene.c.h before their Rust port (#4646). end()
 * asks for the transform through the scene's do_update_transform(), a macro
 * private to obs-scene.c.h; here it is the same dispatch through the live
 * adapter of obs-scene-item-bounds-setters-fields.c.h, which the bounds
 * setters already call. */
void obs_sceneitem_bounds_update_transform(obs_sceneitem_t *item);

#define do_update_transform(item) obs_sceneitem_bounds_update_transform(item)

void obs_sceneitem_defer_update_begin(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_defer_update_begin"))
		return;

	os_atomic_inc_long(&item->defer_update);
}

void obs_sceneitem_defer_update_end(obs_sceneitem_t *item)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_defer_update_end"))
		return;

	if (os_atomic_dec_long(&item->defer_update) == 0)
		do_update_transform(item);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef do_update_transform
