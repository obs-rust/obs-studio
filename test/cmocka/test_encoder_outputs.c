/* C-source characterization of the encoder output-list pair (#5313).
 * Record the exact mutex and list contents at its boundaries, while taking the
 * real mutex. A later Rust swap must relink this test through real adapters. */
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "obs-internal.h"

static struct obs_encoder encoder;
static struct obs_output outputs[4];
static struct obs_output *before[8], *after[8];
static size_t before_count, after_count;
static unsigned lock_calls, unlock_calls;
static bool locked;

static void check_list(struct obs_output *const *expected, size_t count)
{
	assert_int_equal(encoder.outputs.num, count);
	for (size_t i = 0; i < count; i++)
		assert_ptr_equal(encoder.outputs.array[i], expected[i]);
}

int encoder_outputs_lock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &encoder.outputs_mutex);
	assert_false(locked);
	check_list(before, before_count);
	assert_int_equal(pthread_mutex_lock(mutex), 0);
	locked = true;
	lock_calls++;
	return 0;
}

int encoder_outputs_unlock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &encoder.outputs_mutex);
	assert_true(locked);
	check_list(after, after_count);
	locked = false;
	unlock_calls++;
	assert_int_equal(pthread_mutex_unlock(mutex), 0);
	return 0;
}

static int setup(void **state)
{
	(void)state;
	memset(&encoder, 0, sizeof(encoder));
	assert_int_equal(pthread_mutex_init(&encoder.outputs_mutex, NULL), 0);
	locked = false;
	lock_calls = unlock_calls = 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	assert_false(locked);
	da_free(encoder.outputs);
	assert_int_equal(pthread_mutex_destroy(&encoder.outputs_mutex), 0);
	return 0;
}

static void change_output(bool add, struct obs_output *output, struct obs_output *const *expected, size_t count)
{
	before_count = encoder.outputs.num;
	assert_true(before_count <= sizeof(before) / sizeof(before[0]));
	assert_true(count <= sizeof(after) / sizeof(after[0]));
	for (size_t i = 0; i < before_count; i++)
		before[i] = encoder.outputs.array[i];
	for (size_t i = 0; i < count; i++)
		after[i] = expected[i];
	after_count = count;
	lock_calls = unlock_calls = 0;
	if (add)
		obs_encoder_add_output(&encoder, output);
	else
		obs_encoder_remove_output(&encoder, output);
	assert_int_equal(lock_calls, 1);
	assert_int_equal(unlock_calls, 1);
	assert_false(locked);
	check_list(expected, count);
}

static void append_and_remove_exact_output(void **state)
{
	(void)state;
	struct obs_output *a[] = {&outputs[0]};
	struct obs_output *ab[] = {&outputs[0], &outputs[1]};
	struct obs_output *abc[] = {&outputs[0], &outputs[1], &outputs[2]};
	struct obs_output *ac[] = {&outputs[0], &outputs[2]};
	struct obs_output *c[] = {&outputs[2]};
	change_output(true, &outputs[0], a, 1);
	change_output(true, &outputs[1], ab, 2);
	change_output(true, &outputs[2], abc, 3);
	change_output(false, &outputs[1], ac, 2);
	change_output(false, &outputs[0], c, 1);
	change_output(false, &outputs[2], NULL, 0);
}

static void absent_output_is_unchanged(void **state)
{
	(void)state;
	change_output(false, &outputs[3], NULL, 0);
	struct obs_output *ab[] = {&outputs[0], &outputs[1]};
	change_output(true, &outputs[0], ab, 1);
	change_output(true, &outputs[1], ab, 2);
	struct obs_output **array = encoder.outputs.array;
	size_t capacity = encoder.outputs.capacity;
	change_output(false, &outputs[3], ab, 2);
	assert_ptr_equal(encoder.outputs.array, array);
	assert_int_equal(encoder.outputs.capacity, capacity);
}

static void duplicates_remove_first_match_only(void **state)
{
	(void)state;
	/* Duplicate entries and first-match erase are characterized, not endorsed. */
	struct obs_output *aba[] = {&outputs[0], &outputs[1], &outputs[0]};
	struct obs_output *ba[] = {&outputs[1], &outputs[0]};
	struct obs_output *b[] = {&outputs[1]};
	change_output(true, &outputs[0], aba, 1);
	change_output(true, &outputs[1], aba, 2);
	change_output(true, &outputs[0], aba, 3);
	change_output(false, &outputs[0], ba, 2);
	change_output(false, &outputs[0], b, 1);
	change_output(false, &outputs[0], b, 1);
}

static void null_arguments_do_not_lock_or_mutate(void **state)
{
	(void)state;
	/* An uninitialized mutex is safe here: NULL output must return first. */
	struct obs_encoder uninitialized = {0};
	obs_encoder_add_output(NULL, NULL);
	obs_encoder_remove_output(NULL, NULL);
	obs_encoder_add_output(NULL, &outputs[0]);
	obs_encoder_remove_output(NULL, &outputs[0]);
	obs_encoder_add_output(&uninitialized, NULL);
	obs_encoder_remove_output(&uninitialized, NULL);
	assert_int_equal(lock_calls, 0);
	assert_int_equal(unlock_calls, 0);
	assert_null(uninitialized.outputs.array);
	assert_int_equal(uninitialized.outputs.num, 0);
	assert_int_equal(uninitialized.outputs.capacity, 0);
	struct obs_output *a[] = {&outputs[0]};
	change_output(true, &outputs[0], a, 1);
	struct obs_output **array = encoder.outputs.array;
	size_t capacity = encoder.outputs.capacity;
	lock_calls = unlock_calls = 0;
	obs_encoder_add_output(&encoder, NULL);
	obs_encoder_remove_output(&encoder, NULL);
	assert_int_equal(lock_calls, 0);
	assert_int_equal(unlock_calls, 0);
	check_list(a, 1);
	assert_ptr_equal(encoder.outputs.array, array);
	assert_int_equal(encoder.outputs.capacity, capacity);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(append_and_remove_exact_output, setup, teardown),
		cmocka_unit_test_setup_teardown(absent_output_is_unchanged, setup, teardown),
		cmocka_unit_test_setup_teardown(duplicates_remove_first_match_only, setup, teardown),
		cmocka_unit_test_setup_teardown(null_arguments_do_not_lock_or_mutate, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
