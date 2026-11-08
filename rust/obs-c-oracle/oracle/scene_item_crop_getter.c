/* Real headers and the verbatim extraction, never a production getter call. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <limits.h>
#include <errno.h>
#define obs_sceneitem_get_crop oracle_obs_sceneitem_get_crop
#include "libobs/build/libobs.pch.h"
#include "util/threading.h"
#include "scene_item_crop_fields.inc"
#undef obs_sceneitem_get_crop

typedef void (*crop_getter)(const obs_sceneitem_t *, struct obs_sceneitem_crop *);
size_t oracle_scene_crop_getter_layout(unsigned field);
unsigned oracle_scene_crop_getter_probe(const int *values, unsigned flags, crop_getter getter);
unsigned oracle_scene_crop_getter_null(crop_getter getter);
unsigned oracle_scene_crop_getter_concurrent(crop_getter getter);

size_t oracle_scene_crop_getter_layout(unsigned field)
{
	const size_t alignment = _Alignof(void *) > _Alignof(int64_t) ? _Alignof(void *) : _Alignof(int64_t);
	const size_t span = offsetof(struct obs_scene_item, crop) + sizeof(struct obs_sceneitem_crop);
	const size_t layout[] = {(span + alignment - 1) / alignment * alignment,
				 alignment,
				 offsetof(struct obs_scene_item, ref),
				 offsetof(struct obs_scene_item, item_render),
				 offsetof(struct obs_scene_item, crop),
				 sizeof(struct obs_sceneitem_crop),
				 _Alignof(struct obs_sceneitem_crop),
				 offsetof(struct obs_sceneitem_crop, left),
				 offsetof(struct obs_sceneitem_crop, top),
				 offsetof(struct obs_sceneitem_crop, right),
				 offsetof(struct obs_sceneitem_crop, bottom)};
	return field < sizeof(layout) / sizeof(layout[0]) ? layout[field] : (size_t)-1;
}

unsigned oracle_scene_crop_getter_probe(const int *values, unsigned flags, crop_getter getter)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	struct obs_scene_item item;
	memset(&item, 0xa5, sizeof(item));
	item.parent = (flags & 1) ? &scene : NULL;
	memcpy(&item.crop, values, sizeof(item.crop));
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	/* Canaries also detect a short/overlong out-parameter copy. */
	struct guarded_crop {
		unsigned char before[16];
		struct obs_sceneitem_crop crop;
		unsigned char after[16];
	} expected, actual;
	memset(&expected, 0x5a, sizeof(expected));
	memset(&actual, 0x5a, sizeof(actual));
	if (pthread_mutex_lock(&scene.video_mutex))
		abort();
	oracle_obs_sceneitem_get_crop(&item, &expected.crop);
	unsigned result = memcmp(before, &item, sizeof(item)) ? 1U : 0U;
	getter(&item, &actual.crop);
	if (memcmp(before, &item, sizeof(item)))
		result |= 2;
	if (memcmp(&expected, &actual, sizeof(actual)))
		result |= 4;
	if (pthread_mutex_unlock(&scene.video_mutex) || pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

struct crop_log {
	unsigned count;
	int level;
	char text[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void crop_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void crop_capture(int level, const char *format, va_list args, void *param)
{
	struct crop_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}

unsigned oracle_scene_crop_getter_null(crop_getter getter)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	unsigned result = 0;
	struct obs_scene_item item = {0};
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	for (unsigned flags = 0; flags < 3; flags++) {
		struct crop_log expected = {0}, actual = {0};
		struct obs_sceneitem_crop c_crop = {1, 2, 3, 4}, rust_crop = c_crop;
		base_set_log_handler(crop_capture, &expected);
		oracle_obs_sceneitem_get_crop(flags == 1 ? &item : NULL, flags == 0 ? &c_crop : NULL);
		base_set_log_handler(crop_capture, &actual);
		getter(flags == 1 ? &item : NULL, flags == 0 ? &rust_crop : NULL);
		if (expected.count != 1 || actual.count != expected.count || actual.level != expected.level ||
		    strcmp(actual.text, expected.text))
			result |= 1;
		if (memcmp(&c_crop, &rust_crop, sizeof(c_crop)) || memcmp(before, &item, sizeof(item)))
			result |= 2;
	}
	base_set_log_handler(previous, param);
	return result;
}

struct crop_read {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	crop_getter getter;
	struct obs_sceneitem_crop crop;
};
static void *crop_read_thread(void *data)
{
	struct crop_read *read = data;
	os_event_signal(read->entered);
	read->getter(read->item, &read->crop);
	os_event_signal(read->finished);
	return NULL;
}
static unsigned crop_concurrent(crop_getter getter)
{
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	item.parent = &scene;
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	struct crop_read read = {.item = &item, .getter = getter};
	if (os_event_init(&read.entered, OS_EVENT_TYPE_AUTO) || os_event_init(&read.finished, OS_EVENT_TYPE_AUTO) ||
	    pthread_mutex_lock(&scene.video_mutex))
		abort();
	pthread_t thread;
	if (pthread_create(&thread, NULL, crop_read_thread, &read) || os_event_timedwait(read.entered, 10000))
		abort();
	int blocked = os_event_timedwait(read.finished, 20);
	item.crop = (struct obs_sceneitem_crop){INT_MIN, INT_MAX, -1, 17};
	if (pthread_mutex_unlock(&scene.video_mutex) || pthread_join(thread, NULL))
		abort();
	unsigned result = blocked != ETIMEDOUT ? 1U : 0U;
	if (memcmp(&read.crop, &item.crop, sizeof(read.crop)))
		result |= 2;
	os_event_destroy(read.finished);
	os_event_destroy(read.entered);
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}
unsigned oracle_scene_crop_getter_concurrent(crop_getter getter)
{
	return crop_concurrent(oracle_obs_sceneitem_get_crop) | crop_concurrent(getter);
}
