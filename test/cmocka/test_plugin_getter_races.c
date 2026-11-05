/* Exercise the real private publication and getter paths without starting
 * video, media decoding or X11. Run with ThreadSanitizer to detect the races
 * reported by #3573. Each target includes one production implementation so
 * private fields stay private; unused plugin callbacks are garbage-collected. */
#include <assert.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>

#if defined(GETTER_SCROLL)
#include "plugins/obs-filters/scroll-filter.c.h"
static struct scroll_filter_data subject = {.size_mutex = PTHREAD_MUTEX_INITIALIZER};
static void write_value(unsigned i)
{
	scroll_filter_set_size(&subject, true, true, 16 + i % 2, 32 + i % 2);
}
static void read_value(void)
{
	uint32_t width = scroll_filter_width(&subject);
	uint32_t height = scroll_filter_height(&subject);
	assert(width == 16 || width == 17);
	assert(height == 32 || height == 33);
}
#elif defined(GETTER_SLIDESHOW)
#include "plugins/image-source/obs-slideshow.c.h"
static struct slideshow subject = {.index_mutex = PTHREAD_MUTEX_INITIALIZER};
static void write_value(unsigned i)
{
	ss_set_current_index(&subject, i % 2);
}
static void read_value(void)
{
	calldata_t cd = {0};
	current_slide_proc(&subject, &cd);
	long long index = calldata_int(&cd, "current_index");
	assert(index == 0 || index == 1);
	calldata_free(&cd);
}
#elif defined(GETTER_SLIDESHOW_MK2)
#include "plugins/image-source/obs-slideshow-mk2.c.h"
static struct slideshow subject = {.index_mutex = PTHREAD_MUTEX_INITIALIZER};
static void write_value(unsigned i)
{
	ss_set_current_index(&subject, i % 2);
}
static void read_value(void)
{
	calldata_t cd = {0};
	current_slide_proc(&subject, &cd);
	long long index = calldata_int(&cd, "current_index");
	assert(index == 0 || index == 1);
	calldata_free(&cd);
}
#elif defined(GETTER_STINGER)
#include "plugins/obs-transitions/transition-stinger.c.h"
static struct stinger_info subject = {.point_mutex = PTHREAD_MUTEX_INITIALIZER};
static void write_value(unsigned i)
{
	stinger_set_transition_point(&subject, i % 2 ? 0.25f : 0.75f);
}
static void read_value(void)
{
	float point = stinger_get_transition_point(&subject);
	float a = stinger_get_fade_multiplier(&subject, true);
	float b = stinger_get_fade_multiplier(&subject, false);
	assert(point == 0.25f || point == 0.75f);
	assert(a > 1.0f && a <= 4.0f);
	assert(b > 1.0f && b <= 4.0f);
}
#elif defined(GETTER_XSHM)
#include "plugins/linux-capture/xshm-input.c.h"
static struct xshm_data subject;
static void write_value(unsigned i)
{
	/* These fields are also read directly by unchanged capture tests. */
	subject.adj_x_org = i % 2;
	subject.adj_y_org = i % 2;
	subject.adj_width = 16 + i % 2;
	subject.adj_height = 32 + i % 2;
}
static void read_value(void)
{
	uint32_t width = xshm_getwidth(&subject);
	uint32_t height = xshm_getheight(&subject);
	int_fast32_t x = subject.adj_x_org;
	int_fast32_t y = subject.adj_y_org;
	assert(width == 16 || width == 17);
	assert(height == 32 || height == 33);
	assert(x == 0 || x == 1);
	assert(y == 0 || y == 1);
}
#else
#error Select a plugin getter case
#endif

static pthread_barrier_t start;

static void *writer(void *unused)
{
	(void)unused;
	pthread_barrier_wait(&start);
	for (unsigned i = 0; i < 20000; i++)
		write_value(i);
	return NULL;
}

static void *reader(void *unused)
{
	(void)unused;
	pthread_barrier_wait(&start);
	for (unsigned i = 0; i < 20000; i++)
		read_value();
	return NULL;
}

int main(void)
{
	pthread_t write_thread;
	pthread_t read_thread;
	write_value(0);
	assert(pthread_barrier_init(&start, NULL, 2) == 0);
	assert(pthread_create(&write_thread, NULL, writer, NULL) == 0);
	assert(pthread_create(&read_thread, NULL, reader, NULL) == 0);
	assert(pthread_join(write_thread, NULL) == 0);
	assert(pthread_join(read_thread, NULL) == 0);
	assert(pthread_barrier_destroy(&start) == 0);
	read_value();
	return 0;
}
