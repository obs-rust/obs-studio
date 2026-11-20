/* C-source characterization of the hidden encoder-group count accessor.
 * Record mutex calls using the real private layout, without threads or waits.
 * NULL groups are outside the C precondition. A Rust swap must relink this
 * unchanged fixture to its replacement and real group-field adapter. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"

static struct obs_encoder_group groups[2];
static struct obs_encoder_group *selected;
static unsigned lock_calls, unlock_calls;
static bool locked, change_on_lock, change_on_unlock;
static uint32_t locked_value, unlocked_value;

int pthread_mutex_lock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &selected->mutex);
	assert_false(locked);
	assert_int_equal(lock_calls, 0);
	assert_int_equal(unlock_calls, 0);
	lock_calls++;
	locked = true;
	if (change_on_lock)
		selected->num_encoders_started = locked_value;
	return 0;
}

int pthread_mutex_unlock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &selected->mutex);
	assert_true(locked);
	assert_int_equal(lock_calls, 1);
	assert_int_equal(unlock_calls, 0);
	unlock_calls++;
	if (change_on_unlock)
		selected->num_encoders_started = unlocked_value;
	locked = false;
	return 0;
}

static uint32_t query(struct obs_encoder_group *group)
{
	selected = group;
	lock_calls = unlock_calls = 0;
	uint32_t result = obs_encoder_group_num_started(group);
	assert_int_equal(lock_calls, 1);
	assert_int_equal(unlock_calls, 1);
	assert_false(locked);
	return result;
}

static int setup(void **state)
{
	(void)state;
	memset(groups, 0, sizeof(groups));
	selected = NULL;
	locked = change_on_lock = change_on_unlock = false;
	lock_calls = unlock_calls = 0;
	locked_value = unlocked_value = 0;
	return 0;
}

static void exact_count_and_group(void **state)
{
	(void)state;
	const uint32_t values[] = {0, 1, 2, 32, UINT32_MAX - 1, UINT32_MAX};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		groups[0].num_encoders_started = values[i];
		groups[1].num_encoders_started = ~values[i];
		/* Unrelated lifecycle state must not clamp or gate the count. */
		groups[0].destroy_on_stop = true;
		groups[1].frame_rate_divisors_lcm = UINT32_MAX;
		unsigned char before[sizeof(groups)];
		memcpy(before, groups, sizeof(groups));
		assert_int_equal(query(&groups[0]), values[i]);
		assert_int_equal(query(&groups[1]), (uint32_t)~values[i]);
		assert_memory_equal(groups, before, sizeof(groups));
	}
}

static void snapshot_inside_mutex_calls(void **state)
{
	(void)state;
	const uint32_t values[] = {0, 1, UINT32_MAX};
	change_on_lock = change_on_unlock = true;
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		locked_value = values[i];
		unlocked_value = ~locked_value;
		groups[0].num_encoders_started = unlocked_value;
		assert_int_equal(query(&groups[0]), locked_value);
		assert_int_equal(groups[0].num_encoders_started, unlocked_value);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup(exact_count_and_group, setup),
					   cmocka_unit_test_setup(snapshot_inside_mutex_calls, setup)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
