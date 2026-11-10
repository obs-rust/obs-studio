/* Characterize the original C setter and private owned-copy helper.
 * The private helper is compiled verbatim from the current C source because
 * it is hidden in libobs. Its checks characterize C only, not a future Rust swap.
 * dup(NULL) has no guard: a non-NULL encoder is its precondition
 * (characterized, not endorsed). The public setter accepts NULL. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "encoder-error-ownership-dup.h"

static void ownership(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	assert_int_equal(pthread_mutex_init(&encoder.last_error_mutex, NULL), 0);
	long baseline = bnum_allocs();
	obs_encoder_set_last_error(NULL, "ignored");
	obs_encoder_set_last_error(NULL, NULL);
	assert_int_equal(bnum_allocs(), baseline);
	assert_null(encoder_error_ownership_dup(&encoder));
	char input[] = {'a', (char)0xff, 'b', 0};
	obs_encoder_set_last_error(&encoder, input);
	assert_ptr_not_equal(encoder.last_error_message, input);
	assert_memory_equal(encoder.last_error_message, input, sizeof(input));
	input[0] = 'z';
	assert_int_equal(encoder.last_error_message[0], 'a');
	assert_int_equal(bnum_allocs(), baseline + 1);
	char *first = encoder_error_ownership_dup(&encoder);
	char *second = encoder_error_ownership_dup(&encoder);
	assert_non_null(first);
	assert_non_null(second);
	assert_ptr_not_equal(first, encoder.last_error_message);
	assert_ptr_not_equal(first, second);
	assert_string_equal(first, second);
	first[0] = 'x';
	assert_int_equal(second[0], 'a');
	assert_int_equal(encoder.last_error_message[0], 'a');
	obs_encoder_set_last_error(&encoder, "replacement");
	assert_string_equal(encoder.last_error_message, "replacement");
	assert_int_equal(bnum_allocs(), baseline + 3);
	assert_int_equal(second[0], 'a');
	bfree(first);
	bfree(second);
	obs_encoder_set_last_error(&encoder, "");
	assert_non_null(encoder.last_error_message);
	char *empty = encoder_error_ownership_dup(&encoder);
	assert_non_null(empty);
	assert_string_equal(empty, "");
	assert_ptr_not_equal(empty, encoder.last_error_message);
	bfree(empty);
	assert_int_equal(bnum_allocs(), baseline + 1);
	obs_encoder_set_last_error(&encoder, NULL);
	assert_null(encoder.last_error_message);
	assert_null(encoder_error_ownership_dup(&encoder));
	assert_int_equal(bnum_allocs(), baseline);
	assert_int_equal(pthread_mutex_destroy(&encoder.last_error_mutex), 0);
}

struct lock_probe {
	struct obs_encoder encoder;
	pthread_mutex_t mutex;
	pthread_cond_t condition;
	bool started, done, setter;
	char *copy;
};

static void *locked_call(void *data)
{
	struct lock_probe *probe = data;
	pthread_mutex_lock(&probe->mutex);
	probe->started = true;
	pthread_cond_signal(&probe->condition);
	pthread_mutex_unlock(&probe->mutex);
	if (probe->setter)
		obs_encoder_set_last_error(&probe->encoder, "after");
	else
		probe->copy = encoder_error_ownership_dup(&probe->encoder);
	pthread_mutex_lock(&probe->mutex);
	probe->done = true;
	pthread_cond_signal(&probe->condition);
	pthread_mutex_unlock(&probe->mutex);
	return NULL;
}

static void locking(void **state)
{
	(void)state;
	for (unsigned setter = 0; setter < 2; setter++) {
		struct lock_probe probe = {0};
		probe.setter = setter != 0;
		assert_int_equal(pthread_mutex_init(&probe.encoder.last_error_mutex, NULL), 0);
		assert_int_equal(pthread_mutex_init(&probe.mutex, NULL), 0);
		assert_int_equal(pthread_cond_init(&probe.condition, NULL), 0);
		obs_encoder_set_last_error(&probe.encoder, "before");
		pthread_mutex_lock(&probe.encoder.last_error_mutex);
		pthread_t thread;
		assert_int_equal(pthread_create(&thread, NULL, locked_call, &probe), 0);
		pthread_mutex_lock(&probe.mutex);
		while (!probe.started)
			pthread_cond_wait(&probe.condition, &probe.mutex);
		struct timespec deadline;
		assert_int_equal(timespec_get(&deadline, TIME_UTC), TIME_UTC);
		deadline.tv_nsec += 50000000;
		if (deadline.tv_nsec >= 1000000000) {
			deadline.tv_sec++;
			deadline.tv_nsec -= 1000000000;
		}
		int result = 0;
		while (!probe.done && result == 0)
			result = pthread_cond_timedwait(&probe.condition, &probe.mutex, &deadline);
		bool blocked = !probe.done && result == ETIMEDOUT;
		pthread_mutex_unlock(&probe.mutex);
		/* Change while holding the same mutex: dup must see the later value. */
		bfree(probe.encoder.last_error_message);
		probe.encoder.last_error_message = bstrdup("under lock");
		pthread_mutex_unlock(&probe.encoder.last_error_mutex);
		assert_int_equal(pthread_join(thread, NULL), 0);
		assert_true(blocked);
		assert_true(probe.done);
		if (probe.setter)
			assert_string_equal(probe.encoder.last_error_message, "after");
		else
			assert_string_equal(probe.copy, "under lock");
		bfree(probe.copy);
		obs_encoder_set_last_error(&probe.encoder, NULL);
		pthread_cond_destroy(&probe.condition);
		pthread_mutex_destroy(&probe.mutex);
		pthread_mutex_destroy(&probe.encoder.last_error_mutex);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(ownership), cmocka_unit_test(locking)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
