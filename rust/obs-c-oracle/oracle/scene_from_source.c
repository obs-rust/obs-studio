/* Compile the original extracted C verbatim under private names. */
#define scene_info oracle_from_source_scene_info
#define group_info oracle_from_source_group_info
#define obs_scene_from_source oracle_obs_scene_from_source
#define obs_group_from_source oracle_obs_group_from_source
#include "libobs/obs-scene-from-source.c.h"
#undef obs_scene_from_source
#undef obs_group_from_source
const struct obs_source_info scene_info = {.id = "scene"};
const struct obs_source_info group_info = {.id = "group"};
#undef scene_info
#undef group_info
#include "scene_from_source_fields.inc"

typedef obs_scene_t *(*from_source_fn)(const obs_source_t *);
unsigned oracle_scene_from_source_probe(const char *id, unsigned bits, from_source_fn scene_getter,
					from_source_fn group_getter);

unsigned oracle_scene_from_source_probe(const char *id, unsigned bits, from_source_fn scene_getter,
					from_source_fn group_getter)
{
	struct obs_source source = {0};
	struct obs_weak_source control = {0};
	struct obs_scene scenes[2] = {0};
	source.context.control = (struct obs_weak_object *)&control;
	control.source = &source;
	control.ref.refs = 17;
	control.ref.weak_refs = 23;
	source.info.id = id;
	source.info.type = (bits & 1) ? OBS_SOURCE_TYPE_SCENE : OBS_SOURCE_TYPE_INPUT;
	source.context.data = (bits & 2) ? &scenes[(bits >> 2) & 1] : NULL;
	unsigned char before[sizeof(source)], control_before[sizeof(control)], scenes_before[sizeof(scenes)];
	memcpy(before, &source, sizeof(source));
	memcpy(control_before, &control, sizeof(control));
	memcpy(scenes_before, scenes, sizeof(scenes));
	if (!scene_getter)
		scene_getter = oracle_obs_scene_from_source;
	if (!group_getter)
		group_getter = oracle_obs_group_from_source;
	unsigned result = 0;
	if (scene_getter(NULL) || group_getter(NULL))
		result |= 1;
	for (unsigned repeat = 0; repeat < 3; repeat++) {
		obs_scene_t *scene = oracle_obs_scene_from_source(&source);
		obs_scene_t *group = oracle_obs_group_from_source(&source);
		if (scene != (strcmp(id, "scene") == 0 ? source.context.data : NULL) ||
		    group != (strcmp(id, "group") == 0 ? source.context.data : NULL))
			result |= 2;
		if (scene_getter(&source) != scene || group_getter(&source) != group)
			result |= 4;
		if (memcmp(before, &source, sizeof(source)) || memcmp(control_before, &control, sizeof(control)) ||
		    memcmp(scenes_before, scenes, sizeof(scenes)))
			result |= 8;
	}
	/* A fresh call borrows the current context, even after it changes. */
	source.context.data = &scenes[((bits >> 2) & 1) ^ 1];
	memcpy(before, &source, sizeof(source));
	if (scene_getter(&source) != oracle_obs_scene_from_source(&source) ||
	    group_getter(&source) != oracle_obs_group_from_source(&source))
		result |= 16;
	if (memcmp(before, &source, sizeof(source)) || memcmp(control_before, &control, sizeof(control)) ||
	    memcmp(scenes_before, scenes, sizeof(scenes)))
		result |= 32;
	return result;
}
