/* Tier 3: the unchanged extracted obs_sceneitem_set_order()
 * (libobs/obs-scene-item-set-order.c.h) over the real private layout, and
 * the production adapters the Rust shim calls
 * (obs-scene-item-set-order-fields.c.h, defined once, here).
 *
 * The helpers both sides end in are static in obs-scene.c.h, which the
 * oracle does not compile: detach_sceneitem() and attach_sceneitem() below
 * are those functions' bodies, recorded; full_lock(), full_unlock() and
 * signal_reorder() record what they are given. The original calls them
 * directly, the shim through the real adapters, so both reach the same
 * definitions. The shim's full lock is the shared real-named stand-in in
 * scene_atomic_update.c, and its scene reference the Rust obs_scene_get_ref()
 * over the real scene source adapter, whose source reference the shared
 * stand-in in scene_atomic_update.c hands to this file while a probe runs.
 *
 * The original asks the anchor by its libobs name, so it runs on the rule
 * hybrid libobs ships (obs-scene's item_order; its own parity is
 * item_order_parity): a movement outside enum obs_order_movement stays on
 * both sides (#2895). */
#include <stdlib.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

enum { SSO_ITEMS = 6, SSO_EVENTS = 32 };

/* Event kinds, in bits 24-31; the item, the other operand and the lock depth
 * at the call fill bits 16-23, 8-15 and 0-7. */
enum {
	SSO_EV_REF = 1,
	SSO_EV_REF_FAILED = 2,
	SSO_EV_LOCK = 3,
	SSO_EV_UNLOCK = 4,
	SSO_EV_DETACH = 5,
	SSO_EV_ATTACH = 6,
	SSO_EV_SIGNAL = 7,
	SSO_EV_RELEASE = 8,
	SSO_EV_FOREIGN = 9, /* a call on something outside the fixture */
};
/* Operand ids: an item is its index. */
enum { SSO_ID_SCENE = 0xe0, SSO_ID_NULL = 0xf0, SSO_ID_OTHER = 0xff };

struct sso_fixture {
	struct obs_scene scene;
	struct obs_source source;
	struct obs_scene_item items[SSO_ITEMS];
	/* Not part of the compared objects: */
	long refs;
	unsigned depth;
	unsigned count;
	uint32_t events[SSO_EVENTS];
};

static _Thread_local struct sso_fixture *sso_active;

static uint32_t sso_item_id(const struct sso_fixture *f, const struct obs_scene_item *item)
{
	if (!item)
		return SSO_ID_NULL;
	for (uint32_t i = 0; i < SSO_ITEMS; i++)
		if (item == &f->items[i])
			return i;
	return SSO_ID_OTHER;
}

static uint32_t sso_scene_id(const struct sso_fixture *f, const struct obs_scene *scene)
{
	if (!scene)
		return SSO_ID_NULL;
	return scene == &f->scene ? SSO_ID_SCENE : SSO_ID_OTHER;
}

static struct sso_fixture *sso_fixture(void)
{
	if (!sso_active)
		abort();
	return sso_active;
}

static void sso_record(struct sso_fixture *f, uint32_t kind, uint32_t a, uint32_t b)
{
	if (f->count < SSO_EVENTS)
		f->events[f->count] = kind << 24 | (a & 0xff) << 16 | (b & 0xff) << 8 | (f->depth & 0xff);
	f->count++;
}

/* The source reference behind obs_scene_get_ref()/obs_scene_release(): the
 * fixture's count, as a source's refs, failing at 0 like a source being
 * destroyed. */
static bool sso_source_ref(struct sso_fixture *f, obs_source_t *source, bool acquire)
{
	uint32_t id = source == &f->source ? SSO_ID_SCENE : SSO_ID_OTHER;
	if (!acquire) {
		f->refs--;
		sso_record(f, SSO_EV_RELEASE, id, 0);
		return true;
	}
	if (!f->refs) {
		sso_record(f, SSO_EV_REF_FAILED, id, 0);
		return false;
	}
	f->refs++;
	sso_record(f, SSO_EV_REF, id, 0);
	return true;
}

/* Called by the shared source-reference stand-ins of scene_atomic_update.c;
 * false when no probe of this thread owns the call. */
bool oracle_scene_set_order_source_ref(obs_source_t *source, bool acquire, bool *acquired);
bool oracle_scene_set_order_source_ref(obs_source_t *source, bool acquire, bool *acquired)
{
	struct sso_fixture *f = sso_active;
	if (!f)
		return false;
	*acquired = sso_source_ref(f, source, acquire);
	return true;
}

/* Called by the shared real-named full-lock stand-ins; false when no probe
 * of this thread owns the call. */
bool oracle_scene_set_order_full_lock(obs_scene_t *scene, bool lock);
bool oracle_scene_set_order_full_lock(obs_scene_t *scene, bool lock)
{
	struct sso_fixture *f = sso_active;
	if (!f)
		return false;
	if (scene != &f->scene) {
		sso_record(f, SSO_EV_FOREIGN, lock, sso_scene_id(f, scene));
		return true;
	}
	if (lock) {
		sso_record(f, SSO_EV_LOCK, SSO_ID_SCENE, 0);
		f->depth++;
	} else {
		if (f->depth)
			f->depth--;
		sso_record(f, SSO_EV_UNLOCK, SSO_ID_SCENE, 0);
	}
	return true;
}

/* The original's scene reference, the body of obs-scene-ref.c.h over the
 * same source reference. */
static obs_scene_t *sso_scene_get_ref(obs_scene_t *scene)
{
	if (!scene)
		return NULL;
	if (sso_source_ref(sso_fixture(), scene->source, true))
		return scene;
	return NULL;
}

static void sso_scene_release(obs_scene_t *scene)
{
	if (scene)
		sso_source_ref(sso_fixture(), scene->source, false);
}

static inline void full_lock(struct obs_scene *scene)
{
	oracle_scene_set_order_full_lock(scene, true);
}

static inline void full_unlock(struct obs_scene *scene)
{
	oracle_scene_set_order_full_lock(scene, false);
}

/* obs-scene.c.h's detach_sceneitem(), unchanged but for the record. */
static inline void detach_sceneitem(struct obs_scene_item *item)
{
	struct sso_fixture *f = sso_fixture();
	sso_record(f, SSO_EV_DETACH, sso_item_id(f, item), sso_scene_id(f, item->parent));

	if (item->prev)
		item->prev->next = item->next;
	else
		item->parent->first_item = item->next;

	if (item->next)
		item->next->prev = item->prev;

	item->parent = NULL;
}

/* obs-scene.c.h's attach_sceneitem(), unchanged but for the record. */
static inline void attach_sceneitem(struct obs_scene *parent, struct obs_scene_item *item, struct obs_scene_item *prev)
{
	struct sso_fixture *f = sso_fixture();
	sso_record(f, SSO_EV_ATTACH, sso_item_id(f, item), sso_item_id(f, prev));
	if (parent != &f->scene)
		sso_record(f, SSO_EV_FOREIGN, SSO_EV_ATTACH, sso_scene_id(f, parent));

	item->prev = prev;
	item->parent = parent;

	if (prev) {
		item->next = prev->next;
		if (prev->next)
			prev->next->prev = item;
		prev->next = item;
	} else {
		item->next = parent->first_item;
		if (parent->first_item)
			parent->first_item->prev = item;
		parent->first_item = item;
	}
}

/* signal_reorder() signals "reorder" on item->parent: recorded with the
 * parent it reads and the lock depth. */
static inline void signal_reorder(struct obs_scene_item *item)
{
	struct sso_fixture *f = sso_fixture();
	sso_record(f, SSO_EV_SIGNAL, sso_item_id(f, item), sso_scene_id(f, item->parent));
}

void oracle_obs_sceneitem_set_order(obs_sceneitem_t *item, enum obs_order_movement movement);
#define obs_sceneitem_set_order oracle_obs_sceneitem_set_order
#define obs_scene_get_ref sso_scene_get_ref
#define obs_scene_release sso_scene_release
#include "scene-item-set-order.inc"
#undef obs_scene_release
#undef obs_scene_get_ref
#undef obs_sceneitem_set_order

/* The real adapters the shim calls, over the helpers above. */
#include "scene_item_set_order_fields.inc"

/* bits: 0-2 linked count (7 is 6), 3-5 target (6 and 7 a NULL item; an item
 * at or past the count is detached: no parent, no links), 6 link in reverse,
 * 7 the scene is being destroyed (no reference), 8-15 the noise byte every
 * other field holds. */
static obs_sceneitem_t *sso_init(struct sso_fixture *f, unsigned bits)
{
	memset(f, (int)((bits >> 8) & 0xff), sizeof(*f));
	size_t count = bits & 7;
	if (count > SSO_ITEMS)
		count = SSO_ITEMS;
	bool reverse = (bits & 0x40) != 0;
	f->scene.source = &f->source;
	for (size_t i = 0; i < SSO_ITEMS; i++) {
		struct obs_scene_item *item = &f->items[i];
		item->parent = i < count ? &f->scene : NULL;
		item->prev = NULL;
		item->next = NULL;
	}
	for (size_t k = 0; k < count; k++) {
		size_t i = reverse ? count - 1 - k : k;
		size_t next = reverse ? i - 1 : i + 1;
		f->items[i].next = k + 1 < count ? &f->items[next] : NULL;
		f->items[i].prev = k ? &f->items[reverse ? i + 1 : i - 1] : NULL;
	}
	f->scene.first_item = count ? &f->items[reverse ? count - 1 : 0] : NULL;
	f->refs = (bits & 0x80) ? 0 : 1;
	f->depth = 0;
	f->count = 0;
	memset(f->events, 0, sizeof(f->events));
	unsigned target = (bits >> 3) & 7;
	return target < SSO_ITEMS ? &f->items[target] : NULL;
}

typedef void (*sso_fn)(obs_sceneitem_t *, int);

struct sso_run {
	long refs;
	unsigned depth;
	unsigned count;
	uint32_t events[SSO_EVENTS];
	unsigned char objects[offsetof(struct sso_fixture, refs)];
};

static void sso_original(obs_sceneitem_t *item, int movement)
{
	oracle_obs_sceneitem_set_order(item, (enum obs_order_movement)movement);
}

static void sso_run(struct sso_fixture *f, unsigned bits, int movement, sso_fn set_order, struct sso_run *out)
{
	obs_sceneitem_t *item = sso_init(f, bits);
	sso_active = f;
	set_order(item, movement);
	sso_active = NULL;
	out->refs = f->refs;
	out->depth = f->depth;
	out->count = f->count;
	memcpy(out->events, f->events, sizeof(out->events));
	memcpy(out->objects, f, sizeof(out->objects));
}

/* Runs the original and `rust` on the same fixture address. Returns 0 when
 * they match: 1 the call trace (references, lock, detach, attach with its
 * anchor, signal with the parent it read, each with the lock depth), 2 any
 * scene, source or item byte (links, parents and noise), 4 the reference
 * count or lock depth left; 8 when the original itself leaves the lock held
 * or a reference taken. */
unsigned oracle_scene_item_set_order_probe(unsigned bits, int movement, sso_fn rust);
unsigned oracle_scene_item_set_order_probe(unsigned bits, int movement, sso_fn rust)
{
	static _Thread_local struct sso_fixture fixture;
	struct sso_run expected, actual;
	sso_run(&fixture, bits, movement, sso_original, &expected);
	sso_run(&fixture, bits, movement, rust, &actual);
	unsigned result = 0;
	if (expected.count != actual.count || memcmp(expected.events, actual.events, sizeof(expected.events)))
		result |= 1;
	if (memcmp(expected.objects, actual.objects, sizeof(expected.objects)))
		result |= 2;
	if (expected.refs != actual.refs || expected.depth != actual.depth)
		result |= 4;
	long refs = (bits & 0x80) ? 0 : 1;
	if (expected.depth || expected.refs != refs)
		result |= 8;
	return result;
}

/* The original's result on the bits' fixture: the list from first_item up
 * as item indexes (SSO_ID_OTHER past the end, after 7 steps a cycle), and
 * the number of signals and of trace events. Returns the list length. */
size_t oracle_scene_item_set_order_list(unsigned bits, int movement, uint8_t order[SSO_ITEMS + 1], unsigned *signals,
					unsigned *events);
size_t oracle_scene_item_set_order_list(unsigned bits, int movement, uint8_t order[SSO_ITEMS + 1], unsigned *signals,
					unsigned *events)
{
	static _Thread_local struct sso_fixture fixture;
	struct sso_run run;
	sso_run(&fixture, bits, movement, sso_original, &run);
	size_t length = 0;
	for (struct obs_scene_item *item = fixture.scene.first_item; item && length <= SSO_ITEMS; item = item->next)
		order[length++] = (uint8_t)sso_item_id(&fixture, item);
	*signals = 0;
	for (unsigned i = 0; i < run.count && i < SSO_EVENTS; i++)
		if (run.events[i] >> 24 == SSO_EV_SIGNAL)
			(*signals)++;
	*events = run.count;
	return length;
}

/* Real header size of the movement argument. */
size_t oracle_scene_item_set_order_movement_size(void);
size_t oracle_scene_item_set_order_movement_size(void)
{
	return sizeof(enum obs_order_movement);
}
