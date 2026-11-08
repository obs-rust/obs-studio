/* Pure scene-rule archive consumers never call these actual-object adapters. */
#include <stdlib.h>
#include "obs-scene-source-fields.h"

struct obs_source *const *obs_scene_source_fields(const struct obs_scene *scene)
{
	(void)scene;
	abort();
}

struct obs_source *const *obs_sceneitem_source_fields(const struct obs_scene_item *item)
{
	(void)item;
	abort();
}
