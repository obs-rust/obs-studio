#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <errno.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

struct blocked_read {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	unsigned getter;
	uint32_t result;
};

static void *read_on_thread(void *data)
{
	struct blocked_read *read = data;
	os_event_signal(read->entered);
	if (read->getter == 0)
		read->result = (uint32_t)obs_sceneitem_get_bounds_type(read->item);
	else if (read->getter == 1)
		read->result = obs_sceneitem_get_bounds_alignment(read->item);
	else
		read->result = obs_sceneitem_get_bounds_crop(read->item);
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
	for (unsigned getter = 0; getter < 3; getter++) {
		struct blocked_read read = {.item = &item, .getter = getter};
		assert_int_equal(os_event_init(&read.entered, OS_EVENT_TYPE_AUTO), 0);
		assert_int_equal(os_event_init(&read.finished, OS_EVENT_TYPE_AUTO), 0);
		item.bounds_type = OBS_BOUNDS_NONE;
		item.bounds_align = 0;
		item.crop_to_bounds = false;
		assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
		pthread_t thread;
		assert_int_equal(pthread_create(&thread, NULL, read_on_thread, &read), 0);
		assert_int_equal(os_event_timedwait(read.entered, 10000), 0);
		/* A started getter cannot finish while another thread owns precisely
		 * the parent's video mutex. The timeout probes non-completion, rather
		 * than sleeping and assuming the getter has produced a result. */
		int blocked = os_event_timedwait(read.finished, 20);
		item.bounds_type = OBS_BOUNDS_STRETCH;
		item.bounds_align = UINT32_MAX;
		item.crop_to_bounds = true;
		assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
		assert_int_equal(pthread_join(thread, NULL), 0);
		assert_int_equal(blocked, ETIMEDOUT);
		assert_int_equal(read.result, getter == 0   ? (uint32_t)OBS_BOUNDS_STRETCH
					      : getter == 1 ? UINT32_MAX
							    : 1);
		os_event_destroy(read.finished);
		os_event_destroy(read.entered);
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

/* Actual-library characterization. The item and its scene are complete C
 * objects; no OBS startup, graphics context or source callbacks are needed. */
static void bounds_getters(void **state)
{
	(void)state;
	assert_int_equal(obs_sceneitem_get_bounds_type(NULL), OBS_BOUNDS_NONE);
	assert_int_equal(obs_sceneitem_get_bounds_alignment(NULL), 0);
	assert_false(obs_sceneitem_get_bounds_crop(NULL));
	struct obs_scene scene = {0};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	for (unsigned parented = 0; parented < 2; parented++) {
		for (unsigned type = OBS_BOUNDS_NONE; type <= OBS_BOUNDS_MAX_ONLY; type++) {
			for (unsigned crop = 0; crop < 2; crop++) {
				struct obs_scene_item item = {0};
				item.parent = parented ? &scene : NULL;
				item.bounds_type = (enum obs_bounds_type)type;
				item.bounds_align = UINT32_MAX - type;
				item.crop_to_bounds = crop != 0;
				unsigned char before[sizeof(item)];
				memcpy(before, &item, sizeof(item));
				/* The production mutex is recursive: an owning caller may read. */
				assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
				assert_int_equal(obs_sceneitem_get_bounds_type(&item), item.bounds_type);
				assert_int_equal(obs_sceneitem_get_bounds_alignment(&item), item.bounds_align);
				assert_int_equal(obs_sceneitem_get_bounds_crop(&item), item.crop_to_bounds);
				assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
				assert_memory_equal(before, &item, sizeof(item));
			}
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(bounds_getters), cmocka_unit_test(parent_video_mutex)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
