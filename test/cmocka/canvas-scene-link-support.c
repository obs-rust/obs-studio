/* Real private adapter for standalone Rust-static consumers. */
#include "obs-internal.h"
#include "obs-scene.h"

void *obs_canvas_scene_context_data(const obs_source_t *source);
void *obs_canvas_scene_context_data(const obs_source_t *source)
{
	return source->context.data;
}

obs_source_t *obs_canvas_scene_source(const obs_scene_t *scene);
obs_source_t *obs_canvas_scene_source(const obs_scene_t *scene)
{
	return scene->source;
}
