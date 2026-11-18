/* The one obs_scene_enum_items() in the oracle library. Rust shims call the
 * scene enumerator by its libobs name, so the parity probes that instrument it
 * (oracle/canvas_move.c, oracle/scene_group_enum.c) share this definition: it
 * hands the call to whichever probe is running on this thread. Outside a probe
 * it does nothing, like enumerating an empty scene. */
#include "obs.h"

typedef bool (*scene_enum_callback)(obs_scene_t *, obs_sceneitem_t *, void *);
bool oracle_canvas_move_enum_items(obs_scene_t *scene, scene_enum_callback callback, void *param);
bool oracle_group_enum_enum_items(obs_scene_t *scene, scene_enum_callback callback, void *param);

void obs_scene_enum_items(obs_scene_t *scene, scene_enum_callback callback, void *param)
{
	if (oracle_canvas_move_enum_items(scene, callback, param))
		return;
	oracle_group_enum_enum_items(scene, callback, param);
}
