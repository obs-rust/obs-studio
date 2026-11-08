/* Pure scene-rule archive consumers never call this actual-object adapter. */
#include <stdlib.h>
#include "obs-scene-item-parent-fields.h"

struct obs_scene *const *obs_sceneitem_parent_fields(const struct obs_scene_item *item)
{
	(void)item;
	abort();
}
