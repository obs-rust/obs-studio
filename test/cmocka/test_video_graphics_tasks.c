/* Characterize the graphics drain through public task dispatch and a real
 * graphics thread. No private queue or replacement task implementation. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <util/threading.h>
#include <pthread.h>

struct graphics_tasks_case {
	os_event_t *entered;
	os_event_t *release;
	os_event_t *request;
	os_event_t *enqueued;
	os_event_t *done;
	int values[8];
	size_t count;
	int callback_wait;
};

struct graphics_tasks_entry {
	struct graphics_tasks_case *test;
	int value;
};

static void record_task(void *param)
{
	struct graphics_tasks_entry *entry = param;
	entry->test->values[entry->test->count++] = entry->value;
}

static void finish_task(void *param)
{
	struct graphics_tasks_case *test = param;
	os_event_signal(test->done);
}

static void block_task(void *param)
{
	struct graphics_tasks_case *test = param;
	os_event_signal(test->entered);
	test->callback_wait = os_event_timedwait(test->release, 10000);
}

static void init_case(struct graphics_tasks_case *test)
{
	assert_int_equal(os_event_init(&test->entered, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(os_event_init(&test->release, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(os_event_init(&test->request, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(os_event_init(&test->enqueued, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(os_event_init(&test->done, OS_EVENT_TYPE_MANUAL), 0);
}

static void free_case(struct graphics_tasks_case *test)
{
	os_event_destroy(test->entered);
	os_event_destroy(test->release);
	os_event_destroy(test->request);
	os_event_destroy(test->enqueued);
	os_event_destroy(test->done);
}

static void graphics_fifo(void **state)
{
	(void)state;
	struct graphics_tasks_case test = {0};
	struct graphics_tasks_entry entries[4];
	init_case(&test);
	obs_queue_task(OBS_TASK_GRAPHICS, block_task, &test, false);
	assert_int_equal(os_event_timedwait(test.entered, 10000), 0);
	for (size_t i = 0; i < 4; i++) {
		entries[i] = (struct graphics_tasks_entry){&test, (int)i + 1};
		obs_queue_task(OBS_TASK_GRAPHICS, record_task, &entries[i], false);
	}
	obs_queue_task(OBS_TASK_GRAPHICS, finish_task, &test, false);
	os_event_signal(test.release);
	assert_int_equal(os_event_timedwait(test.done, 10000), 0);
	assert_int_equal(test.callback_wait, 0);
	assert_int_equal(test.count, 4);
	for (size_t i = 0; i < 4; i++)
		assert_int_equal(test.values[i], (int)i + 1);
	free_case(&test);
}

static void child_task(void *param)
{
	struct graphics_tasks_case *test = param;
	struct graphics_tasks_entry child = {test, 5};
	record_task(&child);
	finish_task(test);
}

static void *enqueue_child(void *param)
{
	struct graphics_tasks_case *test = param;
	if (os_event_timedwait(test->request, 10000) == 0) {
		obs_queue_task(OBS_TASK_GRAPHICS, child_task, test, false);
		os_event_signal(test->enqueued);
	}
	return NULL;
}

static void enqueue_from_callback(void *param)
{
	struct graphics_tasks_case *test = param;
	block_task(test);
	struct graphics_tasks_entry outer = {test, 1};
	struct graphics_tasks_entry nested = {test, 2};
	record_task(&outer);
	/* Characterized: dispatch on the graphics thread is inline. */
	obs_queue_task(OBS_TASK_GRAPHICS, record_task, &nested, false);
	/* A different producer must take task_mutex to append. Its completion
	 * before this callback returns proves the drain released the mutex. */
	os_event_signal(test->request);
	test->callback_wait = os_event_timedwait(test->enqueued, 10000);
	outer.value = 3;
	record_task(&outer);
}

static void graphics_callback_enqueue(void **state)
{
	(void)state;
	struct graphics_tasks_case test = {0};
	struct graphics_tasks_entry tail = {&test, 4};
	pthread_t producer;
	init_case(&test);
	assert_int_equal(pthread_create(&producer, NULL, enqueue_child, &test), 0);
	obs_queue_task(OBS_TASK_GRAPHICS, enqueue_from_callback, &test, false);
	assert_int_equal(os_event_timedwait(test.entered, 10000), 0);
	obs_queue_task(OBS_TASK_GRAPHICS, record_task, &tail, false);
	os_event_signal(test.release);
	assert_int_equal(os_event_timedwait(test.done, 10000), 0);
	assert_int_equal(pthread_join(producer, NULL), 0);
	assert_int_equal(test.callback_wait, 0);
	assert_int_equal(test.count, 5);
	for (size_t i = 0; i < 5; i++)
		assert_int_equal(test.values[i], (int)i + 1);
	free_case(&test);
}

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_video_info info = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_RGBA,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&info) != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		return -1;
	}
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(graphics_fifo),
		cmocka_unit_test(graphics_callback_enqueue),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
