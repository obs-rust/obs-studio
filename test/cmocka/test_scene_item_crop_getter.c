#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <limits.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

struct captured_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct captured_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void null_arguments(void **state)
{
	(void)state;
	log_handler_t previous;
	void *param;
	struct captured_log log = {0};
	struct obs_scene_item item = {0};
	struct obs_sceneitem_crop crop = {1, 2, 3, 4};
	struct obs_sceneitem_crop before = crop;
	base_get_log_handler(&previous, &param);
	base_set_log_handler(capture_log, &log);
	obs_sceneitem_get_crop(NULL, &crop);
	bool item_message = !strcmp(log.message, "obs_sceneitem_get_crop: Null 'item' parameter");
	obs_sceneitem_get_crop(&item, NULL);
	bool crop_message = !strcmp(log.message, "obs_sceneitem_get_crop: Null 'crop' parameter");
	obs_sceneitem_get_crop(NULL, NULL);
	base_set_log_handler(previous, param);
	assert_true(item_message);
	assert_true(crop_message);
	assert_string_equal(log.message, "obs_sceneitem_get_crop: Null 'item' parameter");
	assert_int_equal(log.count, 3);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_memory_equal(&crop, &before, sizeof(crop));
}

/* Complete C-owned objects exercise the actual library without OBS startup.
 * Negative/extreme crops are stored unchanged (characterized, not endorsed). */
static void stored_crop(void **state)
{
	(void)state;
	const int edges[] = {0, -1, 1, INT_MIN, INT_MAX};
	struct obs_scene scene = {0};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	for (unsigned parented = 0; parented < 2; parented++) {
		for (size_t i = 0; i < sizeof(edges) / sizeof(edges[0]); i++) {
			struct obs_scene_item item = {0};
			item.parent = parented ? &scene : NULL;
			item.crop = (struct obs_sceneitem_crop){edges[i], edges[(i + 1) % 5], edges[(i + 2) % 5],
								edges[(i + 3) % 5]};
			unsigned char before[sizeof(item)];
			memcpy(before, &item, sizeof(item));
			struct obs_sceneitem_crop crop = {0};
			assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
			obs_sceneitem_get_crop(&item, &crop);
			assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
			assert_memory_equal(&crop, &item.crop, sizeof(crop));
			assert_memory_equal(before, &item, sizeof(item));
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

struct blocked_read {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	struct obs_sceneitem_crop crop;
};

static void *read_on_thread(void *data)
{
	struct blocked_read *read = data;
	os_event_signal(read->entered);
	obs_sceneitem_get_crop(read->item, &read->crop);
	os_event_signal(read->finished);
	return NULL;
}

static void parent_video_mutex(void **state)
{
	(void)state;
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	item.parent = &scene;
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	struct blocked_read read = {.item = &item};
	assert_int_equal(os_event_init(&read.entered, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&read.finished, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, NULL, read_on_thread, &read), 0);
	assert_int_equal(os_event_timedwait(read.entered, 10000), 0);
	/* Probe non-completion while precisely the parent video mutex is owned. */
	int blocked = os_event_timedwait(read.finished, 20);
	item.crop = (struct obs_sceneitem_crop){INT_MIN, INT_MAX, -1, 17};
	assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
	assert_int_equal(pthread_join(thread, NULL), 0);
	assert_int_equal(blocked, ETIMEDOUT);
	assert_memory_equal(&read.crop, &item.crop, sizeof(read.crop));
	os_event_destroy(read.finished);
	os_event_destroy(read.entered);
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_arguments), cmocka_unit_test(stored_crop),
					   cmocka_unit_test(parent_video_mutex)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
