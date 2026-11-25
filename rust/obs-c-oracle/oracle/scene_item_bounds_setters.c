/* The verbatim extracted bounds setters over real headers and the shared item
 * fields lock (provided by scene_item_bounds_getters.c). The two scene
 * statics they reach through obs-scene-item-bounds-setters-fields.c.h are
 * recording stand-ins here, shared by the C and the Rust setters: each call
 * is logged with whether the parent's video mutex was held at the time. */
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define obs_sceneitem_set_bounds_type oracle_obs_sceneitem_set_bounds_type
#define obs_sceneitem_set_bounds_alignment oracle_obs_sceneitem_set_bounds_alignment
#define obs_sceneitem_set_bounds_crop oracle_obs_sceneitem_set_bounds_crop
#define obs_sceneitem_set_bounds oracle_obs_sceneitem_set_bounds
#define obs_scene_bounds_valid_type oracle_obs_scene_bounds_valid_type
#include "libobs/build/libobs.pch.h"
#include "util/threading.h"
#include "libobs/obs-scene-item-bounds-setters.c.h"
#undef obs_scene_bounds_valid_type
#undef obs_sceneitem_set_bounds
#undef obs_sceneitem_set_bounds_crop
#undef obs_sceneitem_set_bounds_alignment
#undef obs_sceneitem_set_bounds_type

typedef void (*type_setter)(obs_sceneitem_t *, enum obs_bounds_type);
typedef void (*align_setter)(obs_sceneitem_t *, uint32_t);
typedef void (*crop_setter)(obs_sceneitem_t *, bool);
typedef void (*size_setter)(obs_sceneitem_t *, const struct vec2 *);
struct bounds_setters {
	type_setter set_type;
	align_setter set_align;
	crop_setter set_crop;
	size_setter set_size;
};

uint32_t oracle_scene_bounds_valid_type(int64_t type);
size_t oracle_scene_bounds_setters_layout(unsigned field);
unsigned oracle_scene_bounds_setters_probe(unsigned which, uint32_t value, const float *size, unsigned flags,
					   const struct bounds_setters *rust);
unsigned oracle_scene_bounds_setters_null(const struct bounds_setters *rust);
unsigned oracle_scene_bounds_setters_null_bounds(size_setter setter);
unsigned oracle_scene_bounds_setters_concurrent(unsigned which, const struct bounds_setters *rust);

uint32_t oracle_scene_bounds_valid_type(int64_t type)
{
	return (uint32_t)oracle_obs_scene_bounds_valid_type(type);
}

size_t oracle_scene_bounds_setters_layout(unsigned field)
{
	const size_t layout[] = {
		offsetof(struct obs_scene_item, absolute_coordinates), offsetof(struct obs_scene_item, bounds_type),
		offsetof(struct obs_scene_item, bounds_align),         offsetof(struct obs_scene_item, bounds),
		offsetof(struct obs_scene_item, crop_to_bounds),       sizeof(struct vec2),
	};
	return field < sizeof(layout) / sizeof(layout[0]) ? layout[field] : (size_t)-1;
}

/* Every item the setters see is the `item` of one of these, so the stand-ins
 * find their log without any shared state. */
enum { BOUNDS_EVENTS = 4 };
struct bounds_event {
	unsigned kind;      /* 1: size_from_absolute, 2: update_transform */
	unsigned held;      /* the parent's video mutex, seen from another thread */
	unsigned dst;       /* 1: &item->bounds */
	unsigned src;       /* 1: the caller's size */
	uint32_t alignment; /* Stored alignment at the dependency call. */
	struct vec2 value;
};
struct bounds_item {
	struct obs_scene_item item;
	const struct vec2 *src;
	unsigned count;
	struct bounds_event events[BOUNDS_EVENTS];
};

static struct bounds_item *bounds_item_of(const obs_sceneitem_t *item)
{
	return (struct bounds_item *)(uintptr_t)((const char *)item - offsetof(struct bounds_item, item));
}

/* A recursive mutex lets its owner relock it, so ask from another thread. */
static void *bounds_trylock(void *data)
{
	pthread_mutex_t *mutex = data;
	if (pthread_mutex_trylock(mutex))
		return mutex;
	if (pthread_mutex_unlock(mutex))
		abort();
	return NULL;
}

static unsigned bounds_held(const obs_sceneitem_t *item)
{
	if (!item->parent)
		return 2;
	pthread_t probe;
	void *held = NULL;
	if (pthread_create(&probe, NULL, bounds_trylock, &item->parent->video_mutex) || pthread_join(probe, &held))
		abort();
	return held ? 1 : 0;
}

static struct bounds_event *bounds_record(const obs_sceneitem_t *item, unsigned kind)
{
	struct bounds_item *owner = bounds_item_of(item);
	if (owner->count >= BOUNDS_EVENTS)
		abort();
	struct bounds_event *event = &owner->events[owner->count++];
	event->kind = kind;
	event->alignment = item->align;
	event->held = bounds_held(item);
	return event;
}

void obs_sceneitem_bounds_size_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_bounds_update_transform(obs_sceneitem_t *item);

void obs_sceneitem_bounds_size_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	struct bounds_event *event = bounds_record(item, 1);
	event->dst = dst == &item->bounds;
	event->src = v == bounds_item_of(item)->src;
	event->value = *v;
	/* Any conversion will do: both setters go through this one. */
	dst->x = v->x * 0.5f - 3.0f;
	dst->y = v->y / 1080.0f;
}

void obs_sceneitem_bounds_update_transform(obs_sceneitem_t *item)
{
	bounds_record(item, 2);
}

static void bounds_item_init(struct bounds_item *probe, struct obs_scene *parent, unsigned flags)
{
	memset(probe, 0xa5, sizeof(*probe));
	probe->item.parent = parent;
	probe->item.absolute_coordinates = (flags & 2) != 0;
	probe->src = NULL;
	probe->count = 0;
	memset(probe->events, 0, sizeof(probe->events));
}

static void bounds_call(const struct bounds_setters *setters, unsigned which, obs_sceneitem_t *item, uint32_t value,
			const struct vec2 *size)
{
	switch (which) {
	case 0:
		setters->set_type(item, (enum obs_bounds_type)value);
		break;
	case 1:
		setters->set_align(item, value);
		break;
	case 2:
		setters->set_crop(item, value != 0);
		break;
	default:
		setters->set_size(item, size);
		break;
	}
}

static const struct bounds_setters oracle_setters = {
	oracle_obs_sceneitem_set_bounds_type,
	oracle_obs_sceneitem_set_bounds_alignment,
	oracle_obs_sceneitem_set_bounds_crop,
	oracle_obs_sceneitem_set_bounds,
};

/* which: 0 type, 1 alignment, 2 crop (value != 0), 3 size. flags bit 0: a
 * parent scene; bit 1: absolute coordinates; bit 2: the caller already holds
 * the parent's (recursive) video mutex; bit 3: the size is the item's own
 * bounds. Returns 0 when the whole items, the logs and the input match. */
unsigned oracle_scene_bounds_setters_probe(unsigned which, uint32_t value, const float *size, unsigned flags,
					   const struct bounds_setters *rust)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	struct obs_scene *parent = (flags & 1) ? &scene : NULL;
	struct bounds_item *expected = malloc(sizeof(*expected));
	struct bounds_item *actual = malloc(sizeof(*actual));
	if (!expected || !actual)
		abort();
	bounds_item_init(expected, parent, flags);
	bounds_item_init(actual, parent, flags);
	struct vec2 input_e, input_a;
	memcpy(&input_e, size, sizeof(input_e));
	memcpy(&input_a, size, sizeof(input_a));
	if (flags & 8) {
		expected->item.bounds = input_e;
		actual->item.bounds = input_a;
	}
	const struct vec2 *src_e = (flags & 8) ? &expected->item.bounds : &input_e;
	const struct vec2 *src_a = (flags & 8) ? &actual->item.bounds : &input_a;
	expected->src = src_e;
	actual->src = src_a;

	if ((flags & 4) && pthread_mutex_lock(&scene.video_mutex))
		abort();
	bounds_call(&oracle_setters, which, &expected->item, value, src_e);
	bounds_call(rust, which, &actual->item, value, src_a);
	if ((flags & 4) && pthread_mutex_unlock(&scene.video_mutex))
		abort();

	unsigned result = 0;
	if (memcmp(&expected->item, &actual->item, sizeof(expected->item)))
		result |= 1;
	if (expected->count != actual->count ||
	    memcmp(expected->events, actual->events, sizeof(expected->events[0]) * expected->count))
		result |= 2;
	if (memcmp(&input_e, &input_a, sizeof(input_e)) || memcmp(&input_e, size, sizeof(input_e)))
		result |= 4;
	/* Both paths released the lock: another thread can take it now. */
	if (parent && bounds_held(&expected->item) == 1) {
		result |= 8;
		pthread_mutex_unlock(&scene.video_mutex);
	}
	free(actual);
	free(expected);
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

struct bounds_log {
	unsigned count;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void bounds_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void bounds_capture(int level, const char *format, va_list args, void *param)
{
	struct bounds_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}

/* A NULL item: every setter does nothing and logs nothing, in both. */
unsigned oracle_scene_bounds_setters_null(const struct bounds_setters *rust)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	unsigned result = 0;
	const struct vec2 size = {.x = 3.0f, .y = 4.0f};
	for (unsigned which = 0; which < 5; which++) {
		/* which 4: obs_sceneitem_set_bounds(NULL, NULL). */
		const struct vec2 *input = which == 4 ? NULL : &size;
		struct bounds_log expected = {0}, actual = {0};
		base_set_log_handler(bounds_capture, &expected);
		bounds_call(&oracle_setters, which, NULL, 1, input);
		base_set_log_handler(bounds_capture, &actual);
		bounds_call(rust, which, NULL, 1, input);
		if (expected.count || actual.count)
			result |= 1U << which;
	}
	base_set_log_handler(previous, param);
	return result;
}

struct bounds_null_write {
	struct obs_scene_item *item;
	os_event_t *finished;
	size_setter setter;
};
static void *bounds_null_thread(void *data)
{
	struct bounds_null_write *job = data;
	job->setter(job->item, NULL);
	os_event_signal(job->finished);
	return NULL;
}

/* The intentional difference: the C read through a NULL bounds pointer under
 * the item's lock. The port logs it as obs_ptr_valid does and returns before
 * the lock: the item and its log are untouched, and it finishes while this
 * thread holds the parent's video mutex (a port that took it would wait). */
unsigned oracle_scene_bounds_setters_null_bounds(size_setter setter)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	unsigned result = 0;
	for (unsigned flags = 0; flags < 4; flags++) {
		struct bounds_item *probe = malloc(sizeof(*probe));
		if (!probe)
			abort();
		bounds_item_init(probe, (flags & 1) ? &scene : NULL, flags);
		struct obs_scene_item before = probe->item;
		struct bounds_log log = {0};
		struct bounds_null_write job = {.item = &probe->item, .setter = setter};
		if (os_event_init(&job.finished, OS_EVENT_TYPE_AUTO) || pthread_mutex_lock(&scene.video_mutex))
			abort();
		base_set_log_handler(bounds_capture, &log);
		pthread_t thread;
		if (pthread_create(&thread, NULL, bounds_null_thread, &job))
			abort();
		if (os_event_timedwait(job.finished, 10000))
			result |= 4;
		if (pthread_mutex_unlock(&scene.video_mutex) || pthread_join(thread, NULL))
			abort();
		base_set_log_handler(previous, param);
		if (log.count != 1 || log.level != LOG_DEBUG ||
		    strcmp(log.text, "obs_sceneitem_set_bounds: Null 'bounds' parameter"))
			result |= 1;
		if (memcmp(&before, &probe->item, sizeof(before)) || probe->count)
			result |= 2;
		os_event_destroy(job.finished);
		free(probe);
	}
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

struct bounds_write {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	const struct bounds_setters *setters;
	unsigned which;
	struct vec2 size;
};
static void *bounds_write_thread(void *data)
{
	struct bounds_write *job = data;
	os_event_signal(job->entered);
	bounds_call(job->setters, job->which, job->item, 5, &job->size);
	os_event_signal(job->finished);
	return NULL;
}

/* While the parent's video mutex is taken, a setter stores and updates
 * nothing; once it is released, the setter stores, then updates. */
static unsigned bounds_concurrent(unsigned which, const struct bounds_setters *setters)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	struct bounds_item *probe = malloc(sizeof(*probe));
	if (!probe)
		abort();
	bounds_item_init(probe, &scene, 2);
	struct obs_scene_item before = probe->item;
	struct bounds_write job = {.item = &probe->item,
				   .setters = setters,
				   .which = which,
				   .size = {.x = 7.0f, .y = -9.0f}};
	probe->src = &job.size;
	if (os_event_init(&job.entered, OS_EVENT_TYPE_AUTO) || os_event_init(&job.finished, OS_EVENT_TYPE_AUTO) ||
	    pthread_mutex_lock(&scene.video_mutex))
		abort();
	pthread_t thread;
	if (pthread_create(&thread, NULL, bounds_write_thread, &job) || os_event_timedwait(job.entered, 10000))
		abort();
	int blocked = os_event_timedwait(job.finished, 20);
	bool early = memcmp(&before, &probe->item, sizeof(before)) || probe->count;
	if (pthread_mutex_unlock(&scene.video_mutex) || pthread_join(thread, NULL))
		abort();
	unsigned result = blocked != ETIMEDOUT ? 1U : 0U;
	if (early)
		result |= 2;
	if (!memcmp(&before, &probe->item, sizeof(before)) || probe->count != 1 || probe->events[0].kind != 2 ||
	    probe->events[0].held)
		result |= 4;
	os_event_destroy(job.finished);
	os_event_destroy(job.entered);
	free(probe);
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

unsigned oracle_scene_bounds_setters_concurrent(unsigned which, const struct bounds_setters *rust)
{
	return bounds_concurrent(which, &oracle_setters) | (bounds_concurrent(which, rust) << 4);
}

/* Alignment shares the production lock/update boundary, so reuse this recorder
 * instead of defining a second copy of its symbols in the category binary.
 * flags: bit 0 parent, bit 1 group, bit 2 initial dirty flag. */
typedef void (*alignment_setter)(obs_sceneitem_t *, uint32_t);
unsigned oracle_scene_bounds_alignment_probe(alignment_setter original, alignment_setter rust, uint32_t old,
					     uint32_t value, int32_t defer, unsigned flags);
unsigned oracle_scene_bounds_alignment_probe(alignment_setter original, alignment_setter rust, uint32_t old,
					     uint32_t value, int32_t defer, unsigned flags)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	scene.is_group = (flags & 2) != 0;
	struct obs_scene *parent = (flags & 1) ? &scene : NULL;
	struct bounds_item expected, actual;
	bounds_item_init(&expected, parent, 0);
	expected.item.align = old;
	expected.item.defer_update = defer;
	expected.item.update_transform = (flags & 4) != 0;
	memcpy(&actual, &expected, sizeof(actual));
	original(&expected.item, value);
	rust(&actual.item, value);
	unsigned result = 0;
	if (memcmp(&expected.item, &actual.item, sizeof(expected.item)))
		result |= 1;
	if (expected.count != 1 || actual.count != expected.count ||
	    memcmp(expected.events, actual.events, sizeof(expected.events)))
		result |= 2;
	if (parent && bounds_held(&actual.item))
		result |= 4;
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

/* Deferral calls the same transform adapter. Reuse its recorder so the
 * amalgamated category has one definition and one native fixture layout.
 * Every operation compares the whole item and callback trace, while the
 * caller owns a nonrecursive mutex (begin/end must not acquire it). */
typedef void (*defer_operation)(obs_sceneitem_t *);
unsigned oracle_scene_bounds_defer_probe(long initial, unsigned flags, const unsigned char *ops, size_t count,
					 defer_operation c_begin, defer_operation c_end, defer_operation begin,
					 defer_operation end);

unsigned oracle_scene_bounds_defer_probe(long initial, unsigned flags, const unsigned char *ops, size_t count,
					 defer_operation c_begin, defer_operation c_end, defer_operation begin,
					 defer_operation end)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init(&scene.video_mutex, NULL))
		abort();
	scene.is_group = (flags & 2) != 0;
	struct obs_scene *parent = (flags & 1) ? &scene : NULL;
	struct bounds_item expected, actual;
	bounds_item_init(&expected, parent, 0);
	expected.item.align = 37;
	os_atomic_store_long(&expected.item.defer_update, initial);
	os_atomic_store_bool(&expected.item.update_transform, (flags & 4) != 0);
	memcpy(&actual, &expected, sizeof(actual));
	pthread_mutex_lock(&scene.video_mutex);
	unsigned result = 0;
	for (size_t i = 0; i < count; i++) {
		expected.count = actual.count = 0;
		memset(expected.events, 0, sizeof(expected.events));
		memset(actual.events, 0, sizeof(actual.events));
		if (ops[i]) {
			c_begin(&expected.item);
			begin(&actual.item);
		} else {
			c_end(&expected.item);
			end(&actual.item);
		}
		if (memcmp(&expected.item, &actual.item, sizeof(expected.item)) || expected.count != actual.count ||
		    memcmp(expected.events, actual.events, sizeof(expected.events))) {
			result = (unsigned)i + 1;
			break;
		}
	}
	pthread_mutex_unlock(&scene.video_mutex);
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

/* Transform setter parity reuses this oracle's item-local update recorder,
 * real recursive parent mutex, and independent-thread lock observer. */
typedef void (*xform_setter)(obs_sceneitem_t *, const struct vec2 *);
void oracle_scene_bounds_xform_record(const obs_sceneitem_t *item);
unsigned oracle_scene_bounds_xform_probe(xform_setter original, xform_setter rust, unsigned which, const float *value,
					 int32_t defer, unsigned flags);
void oracle_scene_bounds_xform_record(const obs_sceneitem_t *item)
{
	bounds_record(item, 1);
}
unsigned oracle_scene_bounds_xform_probe(xform_setter original, xform_setter rust, unsigned which, const float *value,
					 int32_t defer, unsigned flags)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	scene.is_group = (flags & 2) != 0;
	struct obs_scene *parent = (flags & 1) ? &scene : NULL;
	struct bounds_item expected, actual;
	bounds_item_init(&expected, parent, 0);
	expected.item.absolute_coordinates = (flags & 4) != 0;
	expected.item.is_group = (flags & 8) != 0;
	expected.item.is_scene = (flags & 16) != 0;
	expected.item.defer_update = defer;
	expected.item.update_transform = (flags & 64) != 0;
	vec2_set(&expected.item.scale_ref, 3840.0f, 2160.0f);
	memcpy(&actual, &expected, sizeof(actual));
	struct vec2 input;
	vec2_set(&input, value[0], value[1]);
	struct vec2 *a = &input, *b = &input;
	if ((flags & 32) && which != 1) {
		a = which == 0 ? &expected.item.pos : &expected.item.scale;
		b = which == 0 ? &actual.item.pos : &actual.item.scale;
		vec2_copy(a, &input);
		vec2_copy(b, &input);
	}
	unsigned result = 0;
	for (unsigned repeat = 0; repeat < 2; repeat++) {
		expected.count = actual.count = 0;
		memset(expected.events, 0, sizeof(expected.events));
		memset(actual.events, 0, sizeof(actual.events));
		original(&expected.item, a);
		rust(&actual.item, b);
		if (memcmp(&expected.item, &actual.item, sizeof(expected.item)))
			result |= 1;
		if (expected.count != actual.count || memcmp(expected.events, actual.events, sizeof(expected.events)))
			result |= 2;
		if (parent && bounds_held(&actual.item))
			result |= 4;
		if (memcmp(&input, value, sizeof(input)))
			result |= 8;
	}
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}
