/* Characterizes the real C source bodies, not a future Rust symbol swap.
 * Dependency seams expose failure cleanup and the locked-mutex handoff. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <errno.h>
#include <cmocka.h>
#include "obs-internal.h"

static bool fail_init;
static size_t init_calls, destroy_calls, free_calls, release_calls;
static struct obs_encoder_group *current_group;
static struct obs_encoder *expected[3];

static int group_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr)
{
	init_calls++;
	assert_null(attr);
	return fail_init ? EAGAIN : pthread_mutex_init(mutex, attr);
}

static int group_mutex_destroy(pthread_mutex_t *mutex)
{
	/* actually_destroy must unlock before destroying the ordinary mutex. */
	assert_int_equal(pthread_mutex_trylock(mutex), 0);
	assert_int_equal(pthread_mutex_unlock(mutex), 0);
	destroy_calls++;
	return pthread_mutex_destroy(mutex);
}

static void group_free(void *ptr)
{
	if (ptr && ptr == current_group) {
		assert_int_equal(destroy_calls, 1);
		assert_int_equal(current_group->encoders.num, 0);
		assert_null(current_group->encoders.array);
		current_group = NULL;
	}
	free_calls++;
	bfree(ptr);
}

static void group_encoder_release(obs_encoder_t *encoder)
{
	assert_true(release_calls < 3);
	assert_ptr_equal(encoder, expected[release_calls]);
	assert_null(encoder->encoder_group);
	/* Releases occur with the group mutex still locked. */
	assert_int_equal(pthread_mutex_trylock(&current_group->mutex), EBUSY);
	assert_int_equal(destroy_calls, 0);
	release_calls++;
}

#define obs_encoder_group_create characterized_group_create
#define obs_encoder_group_actually_destroy characterized_group_actually_destroy
#define obs_encoder_group_destroy characterized_group_destroy
#define pthread_mutex_init group_mutex_init
#define pthread_mutex_destroy group_mutex_destroy
#define obs_encoder_release group_encoder_release
#define bfree group_free
#include "encoder-group-lifecycle-under-test.h"
#undef bfree
#undef obs_encoder_release
#undef pthread_mutex_destroy
#undef pthread_mutex_init
#undef obs_encoder_group_destroy
#undef obs_encoder_group_actually_destroy
#undef obs_encoder_group_create

static void reset_observers(void)
{
	fail_init = false;
	init_calls = destroy_calls = free_calls = release_calls = 0;
	current_group = NULL;
}

static void creation_and_failure(void **state)
{
	(void)state;
	reset_observers();
	long baseline = bnum_allocs();
	fail_init = true;
	assert_null(characterized_group_create());
	assert_int_equal(init_calls, 1);
	assert_int_equal(free_calls, 1);
	assert_int_equal(destroy_calls, 0);
	assert_int_equal(bnum_allocs(), baseline);
	reset_observers();
	current_group = characterized_group_create();
	assert_non_null(current_group);
	assert_int_equal(init_calls, 1);
	assert_int_equal(current_group->num_encoders_started, 0);
	assert_false(current_group->destroy_on_stop);
	assert_int_equal(current_group->encoders.num, 0);
	assert_null(current_group->encoders.array);
	assert_int_equal(current_group->frame_rate_divisors_lcm, 0);
	assert_int_equal(current_group->encoders.capacity, 0);
	assert_int_equal(current_group->start_timestamp, 0);
	assert_int_equal(current_group->reconfigure_request, 0);
	assert_int_equal(current_group->next_pts, 0);
	assert_int_equal(current_group->encoders_updated_next_pts, 0);
	assert_int_equal(current_group->encoders_reconfigured, 0);
	assert_false(current_group->reconfigure_again);
	assert_int_equal(pthread_mutex_trylock(&current_group->mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&current_group->mutex), 0);
	characterized_group_destroy(current_group);
	assert_null(current_group);
	assert_int_equal(destroy_calls, 1);
	assert_int_equal(release_calls, 0);
	assert_int_equal(bnum_allocs(), baseline);
}

static void deferred_and_ordered_teardown(void **state)
{
	(void)state;
	for (size_t count = 0; count <= 3; count++) {
		reset_observers();
		long baseline = bnum_allocs();
		struct obs_encoder encoders[3] = {0};
		current_group = characterized_group_create();
		assert_non_null(current_group);
		for (size_t i = 0; i < count; i++) {
			expected[i] = &encoders[i];
			encoders[i].encoder_group = current_group;
			da_push_back(current_group->encoders, &expected[i]);
		}
		current_group->num_encoders_started = 2;
		long populated = bnum_allocs();
		for (size_t repeat = 0; repeat < 2; repeat++) {
			characterized_group_destroy(current_group);
			assert_true(current_group->destroy_on_stop);
			assert_int_equal(current_group->num_encoders_started, 2);
			assert_int_equal(current_group->encoders.num, count);
			assert_int_equal(release_calls, 0);
			assert_int_equal(destroy_calls, 0);
			assert_int_equal(free_calls, 0);
			assert_int_equal(bnum_allocs(), populated);
			for (size_t i = 0; i < count; i++)
				assert_ptr_equal(encoders[i].encoder_group, current_group);
			assert_int_equal(pthread_mutex_trylock(&current_group->mutex), 0);
			assert_int_equal(pthread_mutex_unlock(&current_group->mutex), 0);
		}
		current_group->num_encoders_started = 0;
		if (count % 2) {
			/* The stop path hands actually_destroy an already locked mutex. */
			assert_int_equal(pthread_mutex_lock(&current_group->mutex), 0);
			characterized_group_actually_destroy(current_group);
		} else {
			characterized_group_destroy(current_group);
		}
		assert_null(current_group);
		assert_int_equal(release_calls, count);
		assert_int_equal(destroy_calls, 1);
		assert_int_equal(bnum_allocs(), baseline);
	}
}

static void null_destroy(void **state)
{
	(void)state;
	reset_observers();
	characterized_group_destroy(NULL);
	assert_int_equal(free_calls, 0);
	assert_int_equal(destroy_calls, 0);
	assert_int_equal(release_calls, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(creation_and_failure),
					   cmocka_unit_test(deferred_and_ordered_teardown),
					   cmocka_unit_test(null_destroy)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
