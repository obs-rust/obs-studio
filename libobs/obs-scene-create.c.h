#include "libobs/build/libobs.pch.h"

/* Defined in obs-scene.c.h in this same unity group. These helpers remain
 * C implementation details; a later Rust swap needs a non-inline boundary. */
static inline obs_scene_t *create_id(obs_canvas_t *canvas, const char *id, const char *name);
static inline obs_scene_t *create_private_id(obs_canvas_t *canvas, const char *id, const char *name);

obs_scene_t *obs_scene_create(const char *name)
{
	return create_id(obs->data.main_canvas, "scene", name);
}

obs_scene_t *obs_scene_create_private(const char *name)
{
	return create_private_id(obs->data.main_canvas, "scene", name);
}
