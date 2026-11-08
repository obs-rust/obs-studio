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
	if (read->getter == 0) {
		float value = obs_sceneitem_get_rot(read->item);
		memcpy(&read->result, &value, sizeof(value));
	} else {
		read->result = obs_sceneitem_get_alignment(read->item);
	}
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
	for (unsigned getter = 0; getter < 2; getter++) {
		struct blocked_read read = {.item = &item, .getter = getter};
		assert_int_equal(os_event_init(&read.entered, OS_EVENT_TYPE_AUTO), 0);
		assert_int_equal(os_event_init(&read.finished, OS_EVENT_TYPE_AUTO), 0);
		item.rot = 0.0f;
		item.align = 0;
		assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
		pthread_t thread;
		assert_int_equal(pthread_create(&thread, NULL, read_on_thread, &read), 0);
		assert_int_equal(os_event_timedwait(read.entered, 10000), 0);
		/* A started getter cannot finish while another thread owns precisely
		 * the parent's video mutex. The timeout probes non-completion, rather
		 * than sleeping and assuming the getter has produced a result. */
		int blocked = os_event_timedwait(read.finished, 20);
		item.rot = -0.0f;
		item.align = UINT32_MAX;
		assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
		assert_int_equal(pthread_join(thread, NULL), 0);
		assert_int_equal(blocked, ETIMEDOUT);
		assert_int_equal(read.result, getter == 0 ? UINT32_C(0x80000000) : UINT32_MAX);
		os_event_destroy(read.finished);
		os_event_destroy(read.entered);
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

/* Actual-library characterization. The item and its scene are complete C
 * objects; no OBS startup, graphics context or source callbacks are needed. */
static void transform_getters(void **state)
{
	(void)state;
	float value = obs_sceneitem_get_rot(NULL);
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	assert_int_equal(bits, 0);
	assert_int_equal(obs_sceneitem_get_alignment(NULL), 0);
	const uint32_t edges[] = {0,          0x80000000, 0x3f800000, 0xbf800000, 0x7f800000,
				  0xff800000, 0x7fc12345, 0xffc54321, 1,          0x7f7fffff};
	struct obs_scene scene = {0};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	for (unsigned parented = 0; parented < 2; parented++) {
		for (size_t i = 0; i < sizeof(edges) / sizeof(edges[0]); i++) {
			struct obs_scene_item item = {0};
			item.parent = parented ? &scene : NULL;
			memcpy(&item.rot, &edges[i], sizeof(item.rot));
			item.align = UINT32_MAX - (uint32_t)i;
			unsigned char before[sizeof(item)];
			memcpy(before, &item, sizeof(item));
			assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
			value = obs_sceneitem_get_rot(&item);
			memcpy(&bits, &value, sizeof(bits));
			assert_int_equal(bits, edges[i]);
			assert_int_equal(obs_sceneitem_get_alignment(&item), item.align);
			assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
			assert_memory_equal(before, &item, sizeof(item));
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(transform_getters), cmocka_unit_test(parent_video_mutex)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
