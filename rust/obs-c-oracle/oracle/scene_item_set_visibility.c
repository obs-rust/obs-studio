/* Compare the original C body with the Rust callback through real field adapters.
 * Record every mutex/queue/atomic/active-child operation, including its order. */
#include <stddef.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"

enum { SV_LOCK = 1, SV_CLEAR, SV_REFS, SV_ADD, SV_REMOVE, SV_SET_REFS, SV_VISIBLE, SV_USER_VISIBLE, SV_UNLOCK };
struct sv_event {
	int kind;
	long value;
};
struct sv_result {
	struct sv_event events[16];
	size_t count;
	bool invalid;
	long refs;
	bool visible;
	bool user_visible;
	size_t queued;
};
static struct sv_result *sv_result;
static struct obs_scene_item *sv_item;
static bool sv_locked;

static void sv_record(int kind, long value)
{
	if (!sv_locked || sv_result->count >= 16) {
		sv_result->invalid = true;
		return;
	}
	sv_result->events[sv_result->count++] = (struct sv_event){kind, value};
}

static int sv_lock(pthread_mutex_t *mutex)
{
	if (sv_locked || mutex != &sv_item->actions_mutex)
		sv_result->invalid = true;
	sv_locked = true;
	sv_record(SV_LOCK, 0);
	return pthread_mutex_lock(mutex);
}

static int sv_unlock(pthread_mutex_t *mutex)
{
	sv_record(SV_UNLOCK, 0);
	sv_locked = false;
	return pthread_mutex_unlock(mutex);
}

static void sv_resize(size_t element_size, struct darray *array, size_t size)
{
	sv_record(SV_CLEAR, (long)size);
	darray_resize(element_size, array, size);
}

static long sv_load_refs(const volatile long *refs)
{
	long value = os_atomic_load_long(refs);
	sv_record(SV_REFS, value);
	return value;
}

static long sv_set_refs(volatile long *refs, long value)
{
	sv_record(SV_SET_REFS, value);
	return os_atomic_set_long(refs, value);
}

static bool sv_set_visible(volatile bool *field, bool value)
{
	sv_record(field == &sv_item->visible ? SV_VISIBLE : SV_USER_VISIBLE, value);
	return os_atomic_set_bool(field, value);
}

static bool sv_add(obs_source_t *parent, obs_source_t *child)
{
	sv_record(SV_ADD, parent == sv_item->parent->source && child == sv_item->source);
	return false; /* The original helper ignores failed active-child insertion. */
}

static void sv_remove(obs_source_t *parent, obs_source_t *child)
{
	sv_record(SV_REMOVE, parent == sv_item->parent->source && child == sv_item->source);
}

#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock sv_lock
#define pthread_mutex_unlock sv_unlock
#define darray_resize sv_resize
#define os_atomic_load_long sv_load_refs
#define os_atomic_set_long sv_set_refs
#define os_atomic_set_bool sv_set_visible
#define obs_source_add_active_child sv_add
#define obs_source_remove_active_child sv_remove
#define obs_sceneitem_set_visibility oracle_obs_sceneitem_set_visibility
void oracle_obs_sceneitem_set_visibility(struct obs_scene_item *item, bool visible);
#include "scene_item_set_visibility.inc"
#undef obs_sceneitem_set_visibility
#include "scene_item_set_visibility_fields.inc"
#undef obs_source_remove_active_child
#undef obs_source_add_active_child
#undef os_atomic_set_bool
#undef os_atomic_set_long
#undef os_atomic_load_long
#undef darray_resize
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

typedef bool (*sv_add_fn)(obs_source_t *, obs_source_t *);
typedef void (*sv_remove_fn)(obs_source_t *, obs_source_t *);
typedef void (*sv_setter)(struct obs_scene_item *, bool, sv_add_fn, sv_remove_fn);

static void sv_original(struct obs_scene_item *item, bool visible, sv_add_fn add, sv_remove_fn remove)
{
	(void)add;
	(void)remove;
	oracle_obs_sceneitem_set_visibility(item, visible);
}

static struct sv_result sv_run(long refs, unsigned flags, unsigned queued, bool visible, sv_setter setter)
{
	struct sv_result result = {0};
	struct obs_source parent_source = {0}, child_source = {0};
	struct obs_scene scene = {0};
	scene.source = &parent_source;
	struct obs_scene_item item = {0};
	item.parent = &scene;
	item.source = &child_source;
	item.active_refs = refs;
	item.visible = (flags & 1) != 0;
	item.user_visible = (flags & 2) != 0;
	item.id = -17;
	item.selected = true;
	item.locked = true;
	if (pthread_mutex_init(&item.actions_mutex, NULL) != 0) {
		result.invalid = true;
		return result;
	}
	for (unsigned i = 0; i < queued; i++) {
		struct item_action action = {.timestamp = i + 100, .visible = (i & 1) != 0};
		da_push_back(item.audio_actions, &action);
	}
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	struct item_action actions[8] = {0};
	if (queued)
		memcpy(actions, item.audio_actions.array, queued * sizeof(*actions));
	sv_result = &result;
	sv_item = &item;
	sv_locked = false;
	setter(&item, visible, sv_add, sv_remove);
	result.refs = item.active_refs;
	result.visible = item.visible;
	result.user_visible = item.user_visible;
	result.queued = item.audio_actions.num;
	if (sv_locked || (queued && memcmp(actions, item.audio_actions.array, queued * sizeof(*actions))))
		result.invalid = true;
	/* Exclude only the fields the helper may write, and the real mutex's bytes. */
	const unsigned char *after = (const unsigned char *)&item;
	memcpy(before + offsetof(struct obs_scene_item, active_refs),
	       after + offsetof(struct obs_scene_item, active_refs), sizeof(item.active_refs));
	memcpy(before + offsetof(struct obs_scene_item, visible), after + offsetof(struct obs_scene_item, visible),
	       sizeof(item.visible));
	memcpy(before + offsetof(struct obs_scene_item, user_visible),
	       after + offsetof(struct obs_scene_item, user_visible), sizeof(item.user_visible));
	memcpy(before + offsetof(struct obs_scene_item, audio_actions) + offsetof(struct darray, num),
	       &item.audio_actions.num, sizeof(item.audio_actions.num));
	memcpy(before + offsetof(struct obs_scene_item, actions_mutex), &item.actions_mutex,
	       sizeof(item.actions_mutex));
	if (memcmp(before, &item, sizeof(item)))
		result.invalid = true;
	da_free(item.audio_actions);
	pthread_mutex_destroy(&item.actions_mutex);
	sv_item = NULL;
	sv_result = NULL;
	return result;
}

unsigned oracle_scene_item_set_visibility_probe(long refs, unsigned flags, unsigned queued, bool visible,
						sv_setter setter);
unsigned oracle_scene_item_set_visibility_probe(long refs, unsigned flags, unsigned queued, bool visible,
						sv_setter setter)
{
	if (queued > 8 || !setter)
		return 1;
	struct sv_result c = sv_run(refs, flags, queued, visible, sv_original);
	struct sv_result rust = sv_run(refs, flags, queued, visible, setter);
	unsigned result = c.invalid || rust.invalid ? 1 : 0;
	if (c.refs != rust.refs || c.visible != rust.visible || c.user_visible != rust.user_visible ||
	    c.queued != rust.queued)
		result |= 2;
	if (c.count != rust.count)
		result |= 4;
	for (size_t i = 0; i < c.count && i < rust.count; i++) {
		if (c.events[i].kind != rust.events[i].kind || c.events[i].value != rust.events[i].value)
			result |= 8;
	}
	return result;
}
