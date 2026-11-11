#include "libobs/build/libobs.pch.h"

/* Original inline helpers for the reference-only constructor translation unit.
 * Other callers retain their original helpers in obs-scene.c.h. */
static inline obs_scene_t *create_id(obs_canvas_t *canvas, const char *id, const char *name)
{
	struct obs_source *source = obs_source_create_canvas(canvas, id, name, NULL, NULL);
	return source->context.data;
}

static inline obs_scene_t *create_private_id(obs_canvas_t *canvas, const char *id, const char *name)
{
	struct obs_source *source = obs_source_create_private_with_canvas(canvas, id, name, NULL);
	return source->context.data;
}
