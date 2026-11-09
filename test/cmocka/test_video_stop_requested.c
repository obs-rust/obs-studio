/* C-source characterization of the private video stop traversal. No graphics
 * thread or video-output worker is started. The query fixture observes the
 * exact order and mutex scope of the unchanged C helper. */
#include <errno.h>
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core stop_core;
static size_t stop_visits;
static unsigned stop_mask;
/* MSVC's C headers lack max_align_t. These tokens are never dereferenced. */
static long double stop_outputs[4];

static bool stop_query(video_t *output)
{
	/* Every query runs with the real mixes mutex held. */
	assert_int_equal(pthread_mutex_trylock(&stop_core.video.mixes_mutex), EBUSY);
	assert_true(stop_visits < 4);
	assert_ptr_equal(output, &stop_outputs[stop_visits]);
	return (stop_mask & (1U << stop_visits++)) != 0;
}

/* The fixture replaces only the global handle and output query. The helper's
 * real pthread lock/unlock, traversal and return value remain unchanged. */
#define obs (&stop_core)
#define video_output_stopped stop_query
#include "libobs/obs-video-stop-requested.c.h"
#undef video_output_stopped
#undef obs

static void stop_all_combinations(void **state)
{
	(void)state;
	struct obs_core_video_mix mixes[4] = {0};
	struct obs_core_video_mix *pointers[4];
	assert_int_equal(pthread_mutex_init(&stop_core.video.mixes_mutex, NULL), 0);
	for (size_t i = 0; i < 4; i++) {
		mixes[i].video = (video_t *)&stop_outputs[i];
		pointers[i] = &mixes[i];
	}
	stop_core.video.mixes.array = pointers;
	/* Include the empty list and every boolean vector up to four mixes. */
	for (size_t count = 0; count <= 4; count++) {
		stop_core.video.mixes.num = count;
		for (unsigned mask = 0; mask < (1U << count); mask++) {
			stop_visits = 0;
			stop_mask = mask;
			assert_int_equal(stop_requested(), mask == (1U << count) - 1);
			/* Characterized, not endorsed: do not short-circuit on false. */
			assert_int_equal(stop_visits, count);
			assert_int_equal(pthread_mutex_trylock(&stop_core.video.mixes_mutex), 0);
			assert_int_equal(pthread_mutex_unlock(&stop_core.video.mixes_mutex), 0);
		}
	}
	stop_core.video.mixes.array = NULL;
	stop_core.video.mixes.num = 0;
	assert_int_equal(pthread_mutex_destroy(&stop_core.video.mixes_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(stop_all_combinations)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
