/* Characterization of the actual-library display size getter (#3911).
 * The mutex and display are caller-owned; no graphics initialization. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include "obs-internal.h"
#include <errno.h>
#include <time.h>

static void test_values(void **state)
{
	(void)state;
	uint32_t width = 123, height = 456;
	obs_display_size(NULL, &width, &height);
	assert_int_equal(width, 0);
	assert_int_equal(height, 0);
	struct obs_display display = {0};
	assert_int_equal(pthread_mutex_init(&display.draw_info_mutex, NULL), 0);
	display.enabled = true;
	display.next_cx = 99;
	display.next_cy = 77;
	display.background_color = UINT32_C(0x89abcdef);
	const uint32_t values[] = {0, 1, UINT32_MAX, UINT32_C(0x80000000), 1920};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); ++i) {
		display.cx = values[i];
		display.cy = ~values[i];
		obs_display_size(&display, &width, &height);
		assert_int_equal(width, display.cx);
		assert_int_equal(height, display.cy);
		obs_display_size(&display, &width, &width);
		assert_int_equal(width, display.cy);
		assert_true(display.enabled);
		assert_int_equal(display.next_cx, 99);
		assert_int_equal(display.next_cy, 77);
		assert_int_equal(display.background_color, UINT32_C(0x89abcdef));
	}
	/* Output aliasing a display field preserves the original store order. */
	display.cx = 123;
	display.cy = 456;
	obs_display_size(&display, &display.cy, &height);
	assert_int_equal(display.cy, 123);
	assert_int_equal(height, 123);
	assert_int_equal(pthread_mutex_destroy(&display.draw_info_mutex), 0);
}

struct contention {
	struct obs_display display;
	pthread_mutex_t mutex;
	pthread_cond_t condition;
	bool entered;
	bool done;
	uint32_t width, height;
};

static void *read_size(void *opaque)
{
	struct contention *c = opaque;
	pthread_mutex_lock(&c->mutex);
	c->entered = true;
	pthread_cond_signal(&c->condition);
	pthread_mutex_unlock(&c->mutex);
	obs_display_size(&c->display, &c->width, &c->height);
	pthread_mutex_lock(&c->mutex);
	c->done = true;
	pthread_cond_signal(&c->condition);
	pthread_mutex_unlock(&c->mutex);
	return NULL;
}

static void test_lock(void **state)
{
	(void)state;
	struct contention c = {0};
	pthread_t thread;
	assert_int_equal(pthread_mutex_init(&c.display.draw_info_mutex, NULL), 0);
	assert_int_equal(pthread_mutex_init(&c.mutex, NULL), 0);
	assert_int_equal(pthread_cond_init(&c.condition, NULL), 0);
	assert_int_equal(pthread_mutex_lock(&c.display.draw_info_mutex), 0);
	assert_int_equal(pthread_mutex_lock(&c.mutex), 0);
	assert_int_equal(pthread_create(&thread, NULL, read_size, &c), 0);
	while (!c.entered)
		assert_int_equal(pthread_cond_wait(&c.condition, &c.mutex), 0);
	struct timespec deadline;
	assert_int_equal(timespec_get(&deadline, TIME_UTC), TIME_UTC);
	deadline.tv_sec += 1;
	int result = 0;
	while (!c.done && result == 0)
		result = pthread_cond_timedwait(&c.condition, &c.mutex, &deadline);
	bool waited = !c.done && result == ETIMEDOUT;
	/* Publish under exactly the lock the getter must acquire. */
	c.display.cx = UINT32_MAX;
	c.display.cy = UINT32_C(0x80000001);
	assert_int_equal(pthread_mutex_unlock(&c.display.draw_info_mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&c.mutex), 0);
	assert_int_equal(pthread_join(thread, NULL), 0);
	assert_true(waited);
	assert_int_equal(c.width, UINT32_MAX);
	assert_int_equal(c.height, UINT32_C(0x80000001));
	assert_int_equal(pthread_cond_destroy(&c.condition), 0);
	assert_int_equal(pthread_mutex_destroy(&c.mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&c.display.draw_info_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(test_values), cmocka_unit_test(test_lock)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
