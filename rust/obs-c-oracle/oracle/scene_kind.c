/* Original extracted predicates, with real private layouts confined to C. */
#define scene_info oracle_kind_scene_info
#define group_info oracle_kind_group_info
#define obs_source_is_scene oracle_obs_source_is_scene
#define obs_source_is_group oracle_obs_source_is_group
#include "libobs/obs-scene-kind.c.h"
#undef obs_source_is_scene
#undef obs_source_is_group
const struct obs_source_info scene_info = {.id = "scene"};
const struct obs_source_info group_info = {.id = "group"};
#undef scene_info
#undef group_info

typedef bool (*kind_fn)(const obs_source_t *);
unsigned oracle_scene_kind_probe(const char *id, unsigned bits, long refs, kind_fn scene_getter, kind_fn group_getter);

unsigned oracle_scene_kind_probe(const char *id, unsigned bits, long refs, kind_fn scene_getter, kind_fn group_getter)
{
	struct obs_source source = {0};
	struct obs_weak_source control = {0};
	struct obs_scene scene = {0};
	source.info.id = id;
	source.info.type = (bits & 1) ? OBS_SOURCE_TYPE_SCENE : OBS_SOURCE_TYPE_INPUT;
	source.context.data = (bits & 2) ? &scene : NULL;
	source.context.control = (struct obs_weak_object *)&control;
	control.source = &source;
	control.ref.refs = refs;
	control.ref.weak_refs = 23;
	unsigned char before[sizeof(source)], control_before[sizeof(control)], scene_before[sizeof(scene)];
	memcpy(before, &source, sizeof(source));
	memcpy(control_before, &control, sizeof(control));
	memcpy(scene_before, &scene, sizeof(scene));
	if (!scene_getter)
		scene_getter = oracle_obs_source_is_scene;
	if (!group_getter)
		group_getter = oracle_obs_source_is_group;
	unsigned result = 0;
	if (scene_getter(NULL) != oracle_obs_source_is_scene(NULL) ||
	    group_getter(NULL) != oracle_obs_source_is_group(NULL))
		result |= 1;
	for (unsigned repeat = 0; repeat < 3; repeat++) {
		if (scene_getter(&source) != oracle_obs_source_is_scene(&source) ||
		    group_getter(&source) != oracle_obs_source_is_group(&source))
			result |= 2;
		if (memcmp(before, &source, sizeof(source)) || memcmp(control_before, &control, sizeof(control)) ||
		    memcmp(scene_before, &scene, sizeof(scene)))
			result |= 4;
	}
	return result;
}
