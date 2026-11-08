/* Unrelated scene-rule tests can pull transition getters from the same Rust
 * archive member. They must never call the private field adapter. */
#include <stdlib.h>
#include "obs-scene-item-transition-fields.h"

struct obs_scene_item_transition_fields obs_scene_item_transition_fields(const obs_sceneitem_t *item)
{
	(void)item;
	abort();
}
