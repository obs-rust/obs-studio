#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-scene.h"

static void scale_getter(void **state)
{
	(void)state;
	struct vec2 out;
	const uint32_t sentinel[2] = {0x7fc01234, 0x80000000};
	memcpy(&out, sentinel, sizeof(out));
	obs_sceneitem_get_scale(NULL, &out);
	assert_memory_equal(&out, sentinel, sizeof(out));
	obs_sceneitem_get_scale(NULL, NULL);
	struct obs_scene scene = {.custom_size = true, .cx = 1920, .cy = 1080};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	const uint32_t edges[] = {0,          0x80000000, 0x3f800000, 0xbf800000, 0x7f800000,
				  0xff800000, 0x7fc01234, 0xffc05678, 1,          0x7f7fffff};
	for (unsigned parented = 0; parented < 2; parented++) {
		for (unsigned mode = 0; mode < 4; mode++) {
			for (size_t i = 0; i < sizeof(edges) / sizeof(*edges); i++) {
				struct obs_scene_item item = {0};
				item.parent = parented ? &scene : NULL;
				item.absolute_coordinates = mode == 0;
				item.is_group = mode == 1;
				item.is_scene = mode == 2;
				/* A detached relative plain item needs OBS startup for the
				 * main canvas; the group/scene keep-scale paths do not. */
				if (!parented && mode == 3)
					continue;
				uint32_t bits[2] = {edges[i], edges[(i + 1) % (sizeof(edges) / sizeof(*edges))]};
				memcpy(&item.scale, bits, sizeof(bits));
				item.scale_ref.y = mode == 3 ? 1080.0f : 0.0f;
				unsigned char before[sizeof(item)];
				memcpy(before, &item, sizeof(item));
				assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
				obs_sceneitem_get_scale(&item, &out);
				assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
				/* Multiplication by one preserves these quiet-NaN payloads. */
				assert_memory_equal(&out, bits, sizeof(out));
				assert_memory_equal(&item, before, sizeof(item));
			}
		}
	}
	struct obs_scene_item relative = {.parent = &scene};
	relative.scale.x = -2.0f;
	relative.scale.y = 3.0f;
	relative.scale_ref.y = 540.0f;
	obs_sceneitem_get_scale(&relative, &out);
	assert_float_equal(out.x, -4.0f, 0.0f);
	assert_float_equal(out.y, 6.0f, 0.0f);
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

struct scale_read {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	struct vec2 out;
};

static void *read_scale(void *data)
{
	struct scale_read *read = data;
	os_event_signal(read->entered);
	obs_sceneitem_get_scale(read->item, &read->out);
	os_event_signal(read->finished);
	return NULL;
}

static void parent_video_mutex(void **state)
{
	(void)state;
	struct obs_scene scene = {0};
	struct obs_scene_item item = {.parent = &scene, .absolute_coordinates = true};
	struct scale_read read = {.item = &item};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	assert_int_equal(os_event_init(&read.entered, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&read.finished, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, NULL, read_scale, &read), 0);
	assert_int_equal(os_event_timedwait(read.entered, 10000), 0);
	int blocked = os_event_timedwait(read.finished, 20);
	item.scale.x = -3.0f;
	item.scale.y = 5.0f;
	assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
	assert_int_equal(pthread_join(thread, NULL), 0);
	assert_int_equal(blocked, ETIMEDOUT);
	assert_memory_equal(&read.out, &item.scale, sizeof(read.out));
	os_event_destroy(read.finished);
	os_event_destroy(read.entered);
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(scale_getter), cmocka_unit_test(parent_video_mutex)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
