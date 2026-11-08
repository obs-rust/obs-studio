/* The renderer-backed crop test calls borrowed getters on real OBS objects.
 * Its Rust archive needs live adapters for the private scene layout. */
#include "util/threading.h"
#include "obs-scene.h"
#include "obs-scene-source-fields.h"

struct obs_source *const *obs_scene_source_fields(const struct obs_scene *scene)
{
	return &scene->source;
}

struct obs_source *const *obs_sceneitem_source_fields(const struct obs_scene_item *item)
{
	return &item->source;
}
