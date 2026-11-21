/* Original extracted order-position wrapper over one recorded full-lock
 * boundary. The Rust shim reaches the same boundary through the real seam
 * names: the shared full-lock definition in scene_atomic_update.c dispatches
 * here while a probe is active on this thread, and the real parent
 * (scene_item_parent_getter.c), link (scene_lookup.c) and head adapters read
 * the fixture.
 *
 * The C reference keeps the legacy attached-item predicate, which always
 * accepts (#2899): the original wrapper runs here with it, so NULL and
 * detached items are undefined on that side and are never given to it. They
 * are covered against the shim alone (oracle_scene_item_order_position_hybrid). */
#include "libobs/build/libobs.pch.h"

bool oracle_obs_scene_item_order_item_attached(bool has_item, bool has_parent);
void oracle_order_position_lock(obs_scene_t *scene);
void oracle_order_position_unlock(obs_scene_t *scene);
int oracle_obs_sceneitem_get_order_position(obs_sceneitem_t *item);
#define obs_sceneitem_get_order_position oracle_obs_sceneitem_get_order_position
#define obs_scene_item_order_item_attached oracle_obs_scene_item_order_item_attached
#define obs_scene_atomic_full_lock oracle_order_position_lock
#define obs_scene_atomic_full_unlock oracle_order_position_unlock
#include "scene-item-order-position.inc"
#undef obs_scene_atomic_full_unlock
#undef obs_scene_atomic_full_lock
#undef obs_scene_item_order_item_attached
#undef obs_sceneitem_get_order_position

/* The real head adapter the shim calls. */
#include "scene_item_order_position_fields.inc"

enum { POSITION_EVENTS = 8, POSITION_ITEMS = 6 };
enum { EV_LOCK = 1, EV_UNLOCK = 2, EV_FOREIGN = 0x100, EV_UNBALANCED = 0x200 };
/* What the lock changes until the matching unlock, as another thread holding
 * the list could between the wrapper's head read and its lock: */
enum {
	CLOBBER_HEAD = 1, /* the scene's first item reads as NULL */
	CLOBBER_LINK = 2, /* the first item's next link reads as NULL */
};

struct position_fixture {
	struct obs_scene scene;
	struct obs_scene_item items[POSITION_ITEMS];
	struct obs_scene_item absent;
	unsigned clobber;
	struct obs_scene_item *saved_first;
	struct obs_scene_item *saved_link_item;
	struct obs_scene_item *saved_link;
	unsigned depth;
	unsigned count;
	unsigned events[POSITION_EVENTS];
};

static _Thread_local struct position_fixture *position_active;

static void position_record(struct position_fixture *f, unsigned event)
{
	if (f->count < POSITION_EVENTS)
		f->events[f->count] = event;
	f->count++;
}

static void position_apply(struct position_fixture *f)
{
	f->saved_first = f->scene.first_item;
	f->saved_link_item = f->scene.first_item;
	f->saved_link = f->saved_link_item ? f->saved_link_item->next : NULL;
	if (f->clobber & CLOBBER_HEAD)
		f->scene.first_item = NULL;
	if ((f->clobber & CLOBBER_LINK) && f->saved_link_item)
		f->saved_link_item->next = NULL;
}

static void position_restore(struct position_fixture *f)
{
	f->scene.first_item = f->saved_first;
	if (f->saved_link_item)
		f->saved_link_item->next = f->saved_link;
}

/* Called by the shared real-named full-lock stand-ins; false when no probe of
 * this thread owns the call, so the next recorder handles it. */
bool oracle_scene_order_position_full_lock(obs_scene_t *scene, bool lock);
bool oracle_scene_order_position_full_lock(obs_scene_t *scene, bool lock)
{
	struct position_fixture *f = position_active;
	if (!f)
		return false;
	unsigned event = lock ? EV_LOCK : EV_UNLOCK;
	if (scene != &f->scene)
		event |= EV_FOREIGN;
	if (lock) {
		if (!f->depth++)
			position_apply(f);
	} else if (f->depth) {
		if (!--f->depth)
			position_restore(f);
	} else {
		event |= EV_UNBALANCED;
	}
	position_record(f, event);
	return true;
}

void oracle_order_position_lock(obs_scene_t *scene)
{
	if (!oracle_scene_order_position_full_lock(scene, true))
		abort();
}

void oracle_order_position_unlock(obs_scene_t *scene)
{
	if (!oracle_scene_order_position_full_lock(scene, false))
		abort();
}

/* bits: 0-2 linked count (7 is 6), 3-5 target (6 and 7 the absent item;
 * an unlinked item at or past the count is absent too), 6 link in reverse,
 * 7-8 clobber, 9 vary ids, visibility and group flags. */
static obs_sceneitem_t *position_init(struct position_fixture *f, unsigned bits)
{
	memset(f, 0, sizeof(*f));
	size_t count = bits & 7;
	if (count > POSITION_ITEMS)
		count = POSITION_ITEMS;
	bool reverse = (bits & 0x40) != 0;
	for (size_t i = 0; i < POSITION_ITEMS; i++) {
		struct obs_scene_item *item = &f->items[i];
		item->ref = 17 + (long)i;
		item->id = (bits & 0x200) ? 99 - (int64_t)i : (int64_t)i;
		item->user_visible = (bits & 0x200) && i % 2 == 0;
		item->is_group = (bits & 0x200) && i == 1;
		item->parent = &f->scene;
	}
	f->absent.ref = 23;
	f->absent.parent = &f->scene;
	for (size_t k = 0; k < count; k++) {
		size_t i = reverse ? count - 1 - k : k;
		size_t next = reverse ? i - 1 : i + 1;
		f->items[i].next = k + 1 < count ? &f->items[next] : NULL;
		f->items[i].prev = k ? &f->items[reverse ? i + 1 : i - 1] : NULL;
	}
	f->scene.first_item = count ? &f->items[reverse ? count - 1 : 0] : NULL;
	f->clobber = (bits >> 7) & 3;
	unsigned target = (bits >> 3) & 7;
	return target < POSITION_ITEMS ? &f->items[target] : &f->absent;
}

typedef int (*position_fn)(obs_sceneitem_t *);

struct position_run {
	int result;
	unsigned count;
	unsigned events[POSITION_EVENTS];
	unsigned char objects[offsetof(struct position_fixture, clobber)];
};

static void position_run(struct position_fixture *f, unsigned bits, position_fn getter, struct position_run *out)
{
	obs_sceneitem_t *item = position_init(f, bits);
	position_active = f;
	out->result = getter(item);
	position_active = NULL;
	out->count = f->count;
	memcpy(out->events, f->events, sizeof(out->events));
	memcpy(out->objects, f, sizeof(out->objects));
}

/* Both sides run on the same fixture address with an attached item. Returns 0
 * when the shim matches the original: 1 result, 2 lock trace, 4 any scene or
 * item byte changed (links and refs included), 8 the original's own trace is
 * not exactly lock, unlock. */
unsigned oracle_scene_item_order_position_probe(unsigned bits, position_fn rust);
unsigned oracle_scene_item_order_position_probe(unsigned bits, position_fn rust)
{
	static _Thread_local struct position_fixture fixture;
	struct position_run expected, actual;
	unsigned char before[sizeof(expected.objects)];
	position_init(&fixture, bits);
	memcpy(before, &fixture, sizeof(before));
	position_run(&fixture, bits, oracle_obs_sceneitem_get_order_position, &expected);
	position_run(&fixture, bits, rust, &actual);
	unsigned result = 0;
	if (expected.result != actual.result)
		result |= 1;
	if (expected.count != actual.count || memcmp(expected.events, actual.events, sizeof(expected.events)))
		result |= 2;
	if (memcmp(before, expected.objects, sizeof(before)) || memcmp(before, actual.objects, sizeof(before)))
		result |= 4;
	if (expected.count != 2 || expected.events[0] != EV_LOCK || expected.events[1] != EV_UNLOCK)
		result |= 8;
	return result;
}

/* The original's position for an attached fixture, for value checks. */
int oracle_scene_item_order_position_expected(unsigned bits);
int oracle_scene_item_order_position_expected(unsigned bits)
{
	static _Thread_local struct position_fixture fixture;
	struct position_run run;
	position_run(&fixture, bits, oracle_obs_sceneitem_get_order_position, &run);
	return run.result;
}

/* Hybrid-only inputs, undefined on the C reference: 0 a NULL item, 1 a
 * detached item (NULL parent) that its old scene's list still holds at
 * position 2. Runs the shim alone on the bits' fixture. Returns the result
 * in the low byte, the number of lock events at 8 and 0x10000 when any scene
 * or item byte changed. */
unsigned oracle_scene_item_order_position_hybrid(unsigned which, unsigned bits, position_fn rust);
unsigned oracle_scene_item_order_position_hybrid(unsigned which, unsigned bits, position_fn rust)
{
	static _Thread_local struct position_fixture fixture;
	struct position_fixture *f = &fixture;
	position_init(f, bits);
	obs_sceneitem_t *item = NULL;
	if (which) {
		item = &f->items[2];
		item->parent = NULL;
	}
	unsigned char before[offsetof(struct position_fixture, clobber)];
	memcpy(before, f, sizeof(before));
	position_active = f;
	int position = rust(item);
	position_active = NULL;
	unsigned result = (unsigned)position & 0xff;
	result |= (f->count & 0xff) << 8;
	if (memcmp(before, f, sizeof(before)))
		result |= 0x10000;
	return result;
}

/* Real header size of the returned position. */
size_t oracle_scene_item_order_position_int_size(void);
size_t oracle_scene_item_order_position_int_size(void)
{
	return sizeof(oracle_obs_sceneitem_get_order_position(NULL));
}
