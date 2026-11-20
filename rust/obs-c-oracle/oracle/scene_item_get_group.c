/* Original extracted parent-group wrapper over one recorded lock/walk boundary.
 * The Rust shim reaches the same boundary through the real seam names: the
 * shared full-lock definition in scene_atomic_update.c dispatches here while a
 * probe is active on this thread, and the real group-flag adapter reads the
 * fixture item (scene_item_group_scene.c). */
#include "libobs/build/libobs.pch.h"

void oracle_get_group_lock(obs_scene_t *scene);
void oracle_get_group_unlock(obs_scene_t *scene);
obs_sceneitem_t *oracle_obs_sceneitem_get_group(obs_scene_t *scene, obs_sceneitem_t *group_subitem);
#define obs_sceneitem_get_group oracle_obs_sceneitem_get_group
#define obs_scene_atomic_full_lock oracle_get_group_lock
#define obs_scene_atomic_full_unlock oracle_get_group_unlock
#include "scene-item-get-group.inc"
#undef obs_scene_atomic_full_unlock
#undef obs_scene_atomic_full_lock
#undef obs_sceneitem_get_group

enum { GET_GROUP_EVENTS = 8, GET_GROUP_ITEMS = 4 };
enum { EV_LOCK = 1, EV_LOOKUP = 2, EV_UNLOCK = 3, EV_FOREIGN = 0x100, EV_UNLOCKED_LOOKUP = 0x200 };

struct get_group_fixture {
	struct obs_scene scene;
	struct obs_scene_item item;
	struct obs_scene_item groups[GET_GROUP_ITEMS];
	struct obs_scene_item foreign;
	obs_sceneitem_t *result;
	bool clobber;
	unsigned depth;
	unsigned count;
	unsigned events[GET_GROUP_EVENTS];
};

static _Thread_local struct get_group_fixture *get_group_active;

static void get_group_record(struct get_group_fixture *f, unsigned event)
{
	if (f->count < GET_GROUP_EVENTS)
		f->events[f->count] = event;
	f->count++;
}

/* Called by the shared real-named full-lock stand-ins; false when no probe of
 * this thread owns the call, so the atomic-update recorder handles it. */
bool oracle_scene_get_group_full_lock(obs_scene_t *scene, bool lock);
bool oracle_scene_get_group_full_lock(obs_scene_t *scene, bool lock)
{
	struct get_group_fixture *f = get_group_active;
	if (!f)
		return false;
	unsigned event = lock ? EV_LOCK : EV_UNLOCK;
	if (scene != &f->scene)
		event |= EV_FOREIGN;
	if (lock)
		f->depth++;
	else if (f->depth)
		f->depth--;
	else
		event |= EV_UNLOCKED_LOOKUP;
	get_group_record(f, event);
	return true;
}

void oracle_get_group_lock(obs_scene_t *scene)
{
	if (!oracle_scene_get_group_full_lock(scene, true))
		abort();
}

void oracle_get_group_unlock(obs_scene_t *scene)
{
	if (!oracle_scene_get_group_full_lock(scene, false))
		abort();
}

/* The C-owned walk stays in obs-scene.c.h; both sides reach this stand-in by
 * its real name and get the fixture's chosen, borrowed result. */
obs_sceneitem_t *obs_sceneitem_parent_group_lookup(obs_scene_t *scene, obs_sceneitem_t *group_subitem)
{
	struct get_group_fixture *f = get_group_active;
	if (!f)
		abort();
	unsigned event = EV_LOOKUP | (f->depth << 4);
	if (scene != &f->scene || group_subitem != &f->item)
		event |= EV_FOREIGN;
	if (f->depth != 1)
		event |= EV_UNLOCKED_LOOKUP;
	get_group_record(f, event);
	/* A later read of the group flag would now reject; C reads it once. */
	if (f->clobber)
		f->item.is_group = true;
	return f->result;
}

static void get_group_init(struct get_group_fixture *f, unsigned bits)
{
	memset(f, 0, sizeof(*f));
	f->item.ref = 11;
	f->item.is_group = (bits & 4) != 0;
	f->item.parent = (bits & 0x100) ? NULL : &f->scene;
	f->scene.first_item = &f->groups[0];
	for (size_t i = 0; i < GET_GROUP_ITEMS; i++) {
		f->groups[i].ref = 17 + (long)i;
		f->groups[i].is_group = (bits >> (9 + i)) & 1;
		f->groups[i].removed = (bits & 8) != 0;
		f->groups[i].parent = &f->scene;
		f->groups[i].next = i + 1 < GET_GROUP_ITEMS ? &f->groups[i + 1] : NULL;
	}
	f->foreign.ref = 23;
	f->foreign.is_group = true;
	f->clobber = (bits & 0x10) != 0;
	unsigned choice = (bits >> 5) & 7;
	f->result = choice >= 1 && choice <= GET_GROUP_ITEMS ? &f->groups[choice - 1]
		    : choice == 5                            ? &f->foreign
		    : choice == 6                            ? &f->item
							     : NULL;
}

typedef obs_sceneitem_t *(*get_group_fn)(obs_scene_t *, obs_sceneitem_t *);

struct get_group_run {
	obs_sceneitem_t *result;
	unsigned count;
	unsigned events[GET_GROUP_EVENTS];
	unsigned char objects[offsetof(struct get_group_fixture, result)];
};

static void get_group_run(struct get_group_fixture *f, unsigned bits, get_group_fn getter, struct get_group_run *out)
{
	get_group_init(f, bits);
	get_group_active = f;
	out->result = getter((bits & 1) ? NULL : &f->scene, (bits & 2) ? NULL : &f->item);
	get_group_active = NULL;
	/* Undo the walk's clobber so the bytes show only what the wrapper wrote. */
	f->item.is_group = (bits & 4) != 0;
	out->count = f->count;
	memcpy(out->events, f->events, sizeof(out->events));
	memcpy(out->objects, f, sizeof(out->objects));
}

/* bits: 0 NULL scene, 1 NULL item, 2 group item, 3 removed groups, 4 clobber
 * the flag during the walk, 5-7 walk result, 8 NULL item parent, 9-12 group
 * flags of the scene's items. Both sides run on the same fixture address, so
 * results compare as pointers. Returns 0 when the shim matches the original:
 * 1 result identity, 2 trace, 4 any scene/item byte changed (refs included),
 * 8 the original's own trace is not empty or exactly lock, lookup, unlock. */
unsigned oracle_scene_item_get_group_probe(unsigned bits, get_group_fn rust);
unsigned oracle_scene_item_get_group_probe(unsigned bits, get_group_fn rust)
{
	static _Thread_local struct get_group_fixture fixture;
	struct get_group_run expected, actual;
	unsigned char before[sizeof(expected.objects)];
	get_group_init(&fixture, bits);
	memcpy(before, &fixture, sizeof(before));
	get_group_run(&fixture, bits, oracle_obs_sceneitem_get_group, &expected);
	get_group_run(&fixture, bits, rust, &actual);
	unsigned result = 0;
	if (expected.result != actual.result)
		result |= 1;
	if (expected.count != actual.count || memcmp(expected.events, actual.events, sizeof(expected.events)))
		result |= 2;
	if (memcmp(before, expected.objects, sizeof(before)) || memcmp(before, actual.objects, sizeof(before)))
		result |= 4;
	bool rejected = (bits & 7) != 0;
	if (rejected ? expected.count != 0
		     : expected.count != 3 || expected.events[0] != EV_LOCK ||
			       expected.events[1] != (EV_LOOKUP | 0x10) || expected.events[2] != EV_UNLOCK)
		result |= 8;
	return result;
}

/* Real header layout of the field the shim reads through its adapter. */
size_t oracle_scene_item_get_group_layout(void);
size_t oracle_scene_item_get_group_layout(void)
{
	return sizeof(((struct obs_scene_item *)0)->is_group);
}
