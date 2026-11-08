/* Pure scene-rule archive consumers never call these actual-object adapters. */
#include <stdlib.h>
#include "obs-scene-group-query-fields.h"

const bool *obs_scene_group_query_fields(const struct obs_scene *scene)
{
	(void)scene;
	abort();
}

const bool *obs_sceneitem_group_query_fields(const struct obs_scene_item *item)
{
	(void)item;
	abort();
}
