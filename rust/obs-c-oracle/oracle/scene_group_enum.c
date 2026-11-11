/* Original wrapper over real private layouts. The dependency records exactly
 * which scene/callback/param it receives; Tier 2 exercises the real enumerator. */
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"

void oracle_obs_sceneitem_group_enum_items(obs_sceneitem_t *, bool (*)(obs_scene_t *, obs_sceneitem_t *, void *),
					   void *);
#define obs_sceneitem_group_enum_items oracle_obs_sceneitem_group_enum_items
#include "scene_group_enum.inc"
#undef obs_sceneitem_group_enum_items

typedef bool (*group_enum_callback)(obs_scene_t *, obs_sceneitem_t *, void *);
typedef void (*group_enum_port)(obs_sceneitem_t *, group_enum_callback, void *);
struct group_enum_trace {
	unsigned calls;
	unsigned visits;
	obs_scene_t *scene;
	group_enum_callback callback;
	void *param;
	long ids[3];
	bool valid;
};
struct group_enum_fixture {
	struct obs_scene scenes[2];
	struct obs_scene_item children[3];
	struct group_enum_trace trace;
	unsigned count;
	unsigned stop;
	void *expected_param;
};
#if defined(_MSC_VER)
static __declspec(thread) struct group_enum_fixture *group_enum_current;
#else
static _Thread_local struct group_enum_fixture *group_enum_current;
#endif

static bool group_enum_record(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	struct group_enum_fixture *f = group_enum_current;
	unsigned i = f->trace.visits++;
	if (i >= 3) {
		f->trace.valid = false;
		return false;
	}
	f->trace.valid = f->trace.valid && scene == f->trace.scene && param == f->expected_param &&
			 item == &f->children[i];
	f->trace.ids[i] = (long)item->id; /* fixture ids originate from a long */
	return f->trace.visits != f->stop;
}

/* Instrumented dependency, shared by the original wrapper and Rust FFI. It is
 * not a second implementation of scene snapshot/lifetime management. */
void obs_scene_enum_items(obs_scene_t *scene, group_enum_callback callback, void *param)
{
	struct group_enum_fixture *f = group_enum_current;
	f->trace.calls++;
	f->trace.scene = scene;
	f->trace.callback = callback;
	f->trace.param = param;
	if (callback)
		for (unsigned i = 0; i < f->count; i++)
			if (!callback(scene, &f->children[i], param))
				break;
}

unsigned oracle_scene_group_enum_probe(unsigned bits, long id, group_enum_port port);
unsigned oracle_scene_group_enum_probe(unsigned bits, long id, group_enum_port port)
{
	struct group_enum_fixture f = {0};
	struct obs_source source = {0};
	struct obs_scene_item group = {0};
	group.is_group = (bits & 1) != 0;
	group.selected = (bits & 2) != 0;
	group.removed = (bits & 4) != 0;
	group.id = id;
	source.context.data = (bits & 8) ? &f.scenes[(bits >> 4) & 1] : NULL;
	group.source = (group.is_group || (bits & 2)) ? &source : NULL;
	f.count = (bits >> 5) % 4;
	f.stop = (bits >> 7) % 4;
	f.expected_param = (bits & 512) ? &id : NULL;
	for (unsigned i = 0; i < 3; i++)
		f.children[i].id = id;
	group_enum_callback callback = (bits & 1024) ? NULL : group_enum_record;
	obs_sceneitem_t *item = (bits & 2048) ? NULL : &group;
	unsigned char before_group[sizeof(group)], before_source[sizeof(source)], before_scenes[sizeof(f.scenes)],
		before_children[sizeof(f.children)];
	memcpy(before_group, &group, sizeof(group));
	memcpy(before_source, &source, sizeof(source));
	memcpy(before_scenes, f.scenes, sizeof(f.scenes));
	memcpy(before_children, f.children, sizeof(f.children));
	unsigned result = 0;
	group_enum_current = &f;
	for (unsigned repeat = 0; repeat < 2; repeat++) {
		f.trace = (struct group_enum_trace){.valid = true};
		oracle_obs_sceneitem_group_enum_items(item, callback, f.expected_param);
		struct group_enum_trace expected = f.trace;
		f.trace = (struct group_enum_trace){.valid = true};
		port(item, callback, f.expected_param);
		if (f.trace.calls != expected.calls || f.trace.visits != expected.visits ||
		    f.trace.scene != expected.scene || f.trace.callback != expected.callback ||
		    f.trace.param != expected.param || !f.trace.valid ||
		    memcmp(f.trace.ids, expected.ids, sizeof(expected.ids)))
			result |= 1;
		if (memcmp(before_group, &group, sizeof(group)) || memcmp(before_source, &source, sizeof(source)) ||
		    memcmp(before_scenes, f.scenes, sizeof(f.scenes)) ||
		    memcmp(before_children, f.children, sizeof(f.children)))
			result |= 2;
	}
	group_enum_current = NULL;
	return result;
}
