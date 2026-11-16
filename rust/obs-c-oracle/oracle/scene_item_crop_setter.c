/* The verbatim extracted setter over real headers, the original C clamp and
 * the shared item fields lock (provided by scene_item_bounds_getters.c). */
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define obs_sceneitem_set_crop oracle_obs_sceneitem_set_crop
#define obs_scene_clamp_crop oracle_obs_scene_clamp_crop
#include "libobs/build/libobs.pch.h"
#include "util/threading.h"
#include "libobs/obs-scene-item-crop-setter.c.h"
#undef obs_scene_clamp_crop
#undef obs_sceneitem_set_crop

typedef void (*crop_setter)(obs_sceneitem_t *, const struct obs_sceneitem_crop *);
size_t oracle_scene_crop_setter_layout(unsigned field);
unsigned oracle_scene_crop_setter_probe(const int *stored, const int *input, unsigned flags, crop_setter setter);
unsigned oracle_scene_crop_setter_null(crop_setter setter);
unsigned oracle_scene_crop_setter_concurrent(crop_setter setter);

size_t oracle_scene_crop_setter_layout(unsigned field)
{
	const size_t layout[] = {offsetof(struct obs_scene_item, update_transform), sizeof(bool)};
	return field < sizeof(layout) / sizeof(layout[0]) ? layout[field] : (size_t)-1;
}

/* A recursive mutex lets its owner relock it, so ask from another thread. */
static void *crop_setter_trylock(void *data)
{
	pthread_mutex_t *mutex = data;
	if (pthread_mutex_trylock(mutex))
		return mutex;
	if (pthread_mutex_unlock(mutex))
		abort();
	return NULL;
}

static void crop_setter_item(struct obs_scene_item *item, struct obs_scene *parent, const int *stored, bool dirty)
{
	memset(item, 0xa5, sizeof(*item));
	item->parent = parent;
	memcpy(&item->crop, stored, sizeof(item->crop));
	item->update_transform = dirty;
}

/* flags bit 0: a parent scene; bit 1: update_transform starts set; bit 2:
 * the caller already holds the parent's (recursive) video mutex. */
unsigned oracle_scene_crop_setter_probe(const int *stored, const int *input, unsigned flags, crop_setter setter)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	struct obs_scene *parent = (flags & 1) ? &scene : NULL;
	struct obs_scene_item expected, actual;
	crop_setter_item(&expected, parent, stored, (flags & 2) != 0);
	crop_setter_item(&actual, parent, stored, (flags & 2) != 0);
	struct obs_sceneitem_crop crop, before;
	memcpy(&crop, input, sizeof(crop));
	before = crop;
	if ((flags & 4) && pthread_mutex_lock(&scene.video_mutex))
		abort();
	oracle_obs_sceneitem_set_crop(&expected, &crop);
	unsigned result = memcmp(&before, &crop, sizeof(crop)) ? 1U : 0U;
	setter(&actual, &crop);
	if (memcmp(&before, &crop, sizeof(crop)))
		result |= 2;
	if (memcmp(&expected, &actual, sizeof(actual)))
		result |= 4;
	if ((flags & 4) && pthread_mutex_unlock(&scene.video_mutex))
		abort();
	/* Both paths released the lock: another thread can take it now. */
	pthread_t probe;
	void *held = NULL;
	if (pthread_create(&probe, NULL, crop_setter_trylock, &scene.video_mutex) || pthread_join(probe, &held))
		abort();
	if (held) {
		result |= 8;
		/* The leaked lock is this thread's: release it so the mutex can go. */
		pthread_mutex_unlock(&scene.video_mutex);
	}
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

struct crop_setter_log {
	unsigned count;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void crop_setter_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void crop_setter_capture(int level, const char *format, va_list args, void *param)
{
	struct crop_setter_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}

unsigned oracle_scene_crop_setter_null(crop_setter setter)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	unsigned result = 0;
	struct obs_scene_item item = {0};
	item.crop = (struct obs_sceneitem_crop){9, 8, 7, 6};
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	const struct obs_sceneitem_crop crop = {-1, 2, 3, 4};
	for (unsigned flags = 0; flags < 3; flags++) {
		struct crop_setter_log expected = {0}, actual = {0};
		obs_sceneitem_t *target = flags == 1 ? &item : NULL;
		const struct obs_sceneitem_crop *input = flags == 0 ? &crop : NULL;
		base_set_log_handler(crop_setter_capture, &expected);
		oracle_obs_sceneitem_set_crop(target, input);
		base_set_log_handler(crop_setter_capture, &actual);
		setter(target, input);
		if (expected.count != 1 || actual.count != expected.count || actual.level != expected.level ||
		    strcmp(actual.text, expected.text))
			result |= 1;
		if (memcmp(before, &item, sizeof(item)))
			result |= 2;
	}
	base_set_log_handler(previous, param);
	return result;
}

struct crop_write {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	crop_setter setter;
	struct obs_sceneitem_crop crop;
};
static void *crop_write_thread(void *data)
{
	struct crop_write *job = data;
	os_event_signal(job->entered);
	job->setter(job->item, &job->crop);
	os_event_signal(job->finished);
	return NULL;
}
static unsigned crop_setter_concurrent(crop_setter setter)
{
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	item.parent = &scene;
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	struct crop_write job = {.item = &item, .setter = setter, .crop = {-5, INT_MAX, 7, INT_MIN}};
	if (os_event_init(&job.entered, OS_EVENT_TYPE_AUTO) || os_event_init(&job.finished, OS_EVENT_TYPE_AUTO) ||
	    pthread_mutex_lock(&scene.video_mutex))
		abort();
	pthread_t thread;
	if (pthread_create(&thread, NULL, crop_write_thread, &job) || os_event_timedwait(job.entered, 10000))
		abort();
	int blocked = os_event_timedwait(job.finished, 20);
	/* While the setter waits for the lock, nothing is stored or marked. */
	bool early = os_atomic_load_bool(&item.update_transform) || item.crop.left || item.crop.top ||
		     item.crop.right || item.crop.bottom;
	if (pthread_mutex_unlock(&scene.video_mutex) || pthread_join(thread, NULL))
		abort();
	unsigned result = blocked != ETIMEDOUT ? 1U : 0U;
	if (early)
		result |= 2;
	const struct obs_sceneitem_crop clamped = {0, INT_MAX, 7, 0};
	if (memcmp(&item.crop, &clamped, sizeof(clamped)) || !os_atomic_load_bool(&item.update_transform))
		result |= 4;
	os_event_destroy(job.finished);
	os_event_destroy(job.entered);
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}
unsigned oracle_scene_crop_setter_concurrent(crop_setter setter)
{
	return crop_setter_concurrent(oracle_obs_sceneitem_set_crop) | (crop_setter_concurrent(setter) << 4);
}
