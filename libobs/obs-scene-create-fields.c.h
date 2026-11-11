#include "libobs/build/libobs.pch.h"

/* Borrowed main canvas: unlike obs_get_main_canvas, no reference is acquired. */
obs_canvas_t *obs_scene_create_main_canvas(void);
obs_canvas_t *obs_scene_create_main_canvas(void)
{
	return obs->data.main_canvas;
}

/* Keep source construction and the private source layout on the C side. */
obs_scene_t *obs_scene_create_public_source(obs_canvas_t *canvas, const char *id, const char *name);
obs_scene_t *obs_scene_create_public_source(obs_canvas_t *canvas, const char *id, const char *name)
{
	struct obs_source *source = obs_source_create_canvas(canvas, id, name, NULL, NULL);
	return source->context.data;
}

obs_scene_t *obs_scene_create_private_source(obs_canvas_t *canvas, const char *id, const char *name);
obs_scene_t *obs_scene_create_private_source(obs_canvas_t *canvas, const char *id, const char *name)
{
	struct obs_source *source = obs_source_create_private_with_canvas(canvas, id, name, NULL);
	return source->context.data;
}
