/* Live atomic adapter for standalone Rust-static scene consumers. */
#include "util/threading.h"
#include "obs-scene.h"

bool obs_sceneitem_user_visible_load(const obs_sceneitem_t *item);
bool obs_sceneitem_user_visible_load(const obs_sceneitem_t *item)
{
	return os_atomic_load_bool(&item->user_visible);
}
