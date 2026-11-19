/* Characterize the five public video-statistics queries before the Rust swap
 * (Refs #5171): obs_get_video_frame_time, obs_get_active_fps,
 * obs_get_average_frame_time_ns, obs_get_total_frames and
 * obs_get_lagged_frames. The exact extracted bodies run against stack cores;
 * each must read its own field of obs->video.stats through the matching
 * obs_video_stats_* helper, from whichever core obs points at, and release the
 * stats mutex afterwards. A NULL core is outside the C contract. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

#if !__has_include("libobs/obs-video-stats-queries.c.h")
#error "video-statistics query extraction seam is missing"
#endif

/* The bodies under test read this core, not the library's hidden global. */
struct obs_core *obs;

#define obs_get_video_frame_time frame_time_body
#define obs_get_active_fps active_fps_body
#define obs_get_average_frame_time_ns avg_frame_time_body
#define obs_get_total_frames total_frames_body
#define obs_get_lagged_frames lagged_frames_body
#include "libobs/obs-video-stats-queries.c.h"
#undef obs_get_lagged_frames
#undef obs_get_total_frames
#undef obs_get_average_frame_time_ns
#undef obs_get_active_fps
#undef obs_get_video_frame_time

static struct obs_core core_a, core_b;

struct stats_values {
	uint64_t frame_time_ns;
	uint64_t avg_frame_time_ns;
	double fps;
	uint32_t total_frames;
	uint32_t lagged_frames;
};

static void load(struct obs_core *core, const struct stats_values *v)
{
	core->video.stats.frame_time_ns = v->frame_time_ns;
	core->video.stats.avg_frame_time_ns = v->avg_frame_time_ns;
	core->video.stats.fps = v->fps;
	core->video.stats.total_frames = v->total_frames;
	core->video.stats.lagged_frames = v->lagged_frames;
}

static void *trylock_thread(void *arg)
{
	pthread_mutex_t *mutex = arg;
	int result = pthread_mutex_trylock(mutex);
	if (result == 0)
		pthread_mutex_unlock(mutex);
	return (void *)(intptr_t)result;
}

/* The stats mutex is free again after each query. */
static void assert_unlocked(struct obs_core *core)
{
	pthread_t thread;
	void *result = (void *)(intptr_t)-1;
	assert_int_equal(pthread_create(&thread, NULL, trylock_thread, &core->video.stats.mutex), 0);
	assert_int_equal(pthread_join(thread, &result), 0);
	assert_int_equal((int)(intptr_t)result, 0);
}

static void assert_queries(struct obs_core *core, const struct stats_values *v)
{
	obs = core;
	assert_true(frame_time_body() == v->frame_time_ns);
	assert_unlocked(core);
	assert_true(active_fps_body() == v->fps);
	assert_unlocked(core);
	assert_true(avg_frame_time_body() == v->avg_frame_time_ns);
	assert_unlocked(core);
	assert_true(total_frames_body() == v->total_frames);
	assert_unlocked(core);
	assert_true(lagged_frames_body() == v->lagged_frames);
	assert_unlocked(core);
}

static int setup(void **state)
{
	(void)state;
	memset(&core_a, 0, sizeof(core_a));
	memset(&core_b, 0, sizeof(core_b));
	assert_int_equal(obs_video_stats_init(&core_a.video.stats), 0);
	assert_int_equal(obs_video_stats_init(&core_b.video.stats), 0);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_video_stats_destroy(&core_a.video.stats);
	obs_video_stats_destroy(&core_b.video.stats);
	obs = NULL;
	return 0;
}

/* A fresh core reports zero everywhere. */
static void test_zeroed_stats(void **state)
{
	(void)state;
	static const struct stats_values zero = {0};
	assert_queries(&core_a, &zero);
}

/* Every query reads its own field: all five values are distinct. */
static void test_each_query_reads_its_field(void **state)
{
	(void)state;
	static const struct stats_values v = {
		.frame_time_ns = 0x1122334455667788ULL,
		.avg_frame_time_ns = 16666667ULL,
		.fps = 59.94,
		.total_frames = 123456u,
		.lagged_frames = 7u,
	};
	load(&core_a, &v);
	assert_queries(&core_a, &v);
}

/* Full-width values pass through unchanged (no narrowing or rounding). */
static void test_boundary_values(void **state)
{
	(void)state;
	static const struct stats_values v = {
		.frame_time_ns = UINT64_MAX,
		.avg_frame_time_ns = UINT64_MAX - 1,
		.fps = 1e308,
		.total_frames = UINT32_MAX,
		.lagged_frames = UINT32_MAX - 1,
	};
	load(&core_a, &v);
	assert_queries(&core_a, &v);
}

/* The queries follow the obs pointer: switching cores switches the values. */
static void test_reads_current_core(void **state)
{
	(void)state;
	static const struct stats_values a = {1, 2, 30.0, 4, 5};
	static const struct stats_values b = {10, 20, 0.5, 40, 50};
	load(&core_a, &a);
	load(&core_b, &b);
	assert_queries(&core_a, &a);
	assert_queries(&core_b, &b);
	assert_queries(&core_a, &a);
}

/* Values written through the private setters are the ones read back. */
static void test_reads_setter_writes(void **state)
{
	(void)state;
	obs_video_stats_set_frame_time(&core_a.video.stats, 1000);
	obs_video_stats_set_timing(&core_a.video.stats, 29.97, 33366700ULL);
	obs_video_stats_add_frames(&core_a.video.stats, 10, 3);
	obs_video_stats_add_frames(&core_a.video.stats, 5, 1);
	static const struct stats_values v = {1000, 33366700ULL, 29.97, 15, 4};
	assert_queries(&core_a, &v);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_zeroed_stats, setup, teardown),
		cmocka_unit_test_setup_teardown(test_each_query_reads_its_field, setup, teardown),
		cmocka_unit_test_setup_teardown(test_boundary_values, setup, teardown),
		cmocka_unit_test_setup_teardown(test_reads_current_core, setup, teardown),
		cmocka_unit_test_setup_teardown(test_reads_setter_writes, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
