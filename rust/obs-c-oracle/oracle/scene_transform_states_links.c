/* Test-only link stand-ins: obs_get_source_by_uuid(),
 * obs_sceneitem_set_info2(),
 * obs_sceneitem_defer_update_begin() and obs_sceneitem_defer_update_end()
 * under libobs's own names, for the test
 * binaries that link the obs-scene shims without libobs.
 * obs_sceneitem_get_info2() is the obs-scene shim itself now
 * (rust/obs-scene/src/ffi/item_info_getters.rs).
 *
 * The transform snapshot/restore shims
 * (rust/obs-scene/src/ffi/transform_states.rs) export
 * obs_scene_save_transform_states() and obs_scene_load_transform_states(),
 * whose host calls name these. The MSVC, MinGW and macOS linkers want every
 * symbol an object names resolved, used or not, so obs-tests-core needs a
 * definition; libobs has the real ones.
 *
 * The tests never reach these: they run the shims with a Host of their own,
 * and the parity tests use the oracle's sts_ collaborators
 * (oracle/scene_transform_states.c). */

#include "obs.h"

obs_source_t *obs_get_source_by_uuid(const char *uuid)
{
	UNUSED_PARAMETER(uuid);
	return NULL;
}

void obs_sceneitem_set_info2(obs_sceneitem_t *item, const struct obs_transform_info *info)
{
	UNUSED_PARAMETER(item);
	UNUSED_PARAMETER(info);
}

void obs_sceneitem_defer_update_begin(obs_sceneitem_t *item)
{
	UNUSED_PARAMETER(item);
}

void obs_sceneitem_defer_update_end(obs_sceneitem_t *item)
{
	UNUSED_PARAMETER(item);
}
