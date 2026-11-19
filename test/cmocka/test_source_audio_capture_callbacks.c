/* C-source characterization of registration, not audio delivery (#5236).
 * Real private source/pair layouts and libobs darray allocation; the recording
 * mutex needs no host device or threads. A later Rust swap must link the same
 * assertions through real list/lock adapters (SOURCE_AUDIO_CAPTURE_EXTERNAL). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

static struct obs_source source;
static struct audio_cb_info expected[16];
static size_t expected_count, before_count;
static unsigned locks, unlocks, calls, logs;
static bool locked;
static char message[160];
static int tokens[3];

static void callback_a(void *param, obs_source_t *owner, const struct audio_data *audio, bool muted)
{
	(void)param;
	(void)owner;
	(void)audio;
	(void)muted;
	calls++;
}

static void callback_b(void *param, obs_source_t *owner, const struct audio_data *audio, bool muted)
{
	callback_a(param, owner, audio, muted);
}

static void check_list(void)
{
	assert_int_equal(source.audio_cb_list.num, expected_count);
	for (size_t i = 0; i < expected_count; i++) {
		assert_true(source.audio_cb_list.array[i].callback == expected[i].callback);
		assert_ptr_equal(source.audio_cb_list.array[i].param, expected[i].param);
	}
}

int source_audio_capture_record_lock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &source.audio_cb_mutex);
	assert_false(locked);
	assert_int_equal(source.audio_cb_list.num, before_count);
	locked = true;
	locks++;
	return 0;
}

int source_audio_capture_record_unlock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &source.audio_cb_mutex);
	assert_true(locked);
	check_list(); /* mutation must occur while holding the exact source lock */
	locked = false;
	unlocks++;
	return 0;
}

#ifndef SOURCE_AUDIO_CAPTURE_EXTERNAL
#define pthread_mutex_lock source_audio_capture_record_lock
#define pthread_mutex_unlock source_audio_capture_record_unlock
#include "libobs/obs-source-audio-capture-callbacks.c.h"
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#endif

static void capture_log(int level, const char *format, va_list args, void *param)
{
	assert_int_equal(level, LOG_DEBUG);
	assert_ptr_equal(param, &source);
	logs++;
	vsnprintf(message, sizeof(message), format, args);
}

static int setup(void **state)
{
	(void)state;
	memset(&source, 0, sizeof(source));
	expected_count = before_count = 0;
	locks = unlocks = calls = logs = 0;
	locked = false;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	assert_false(locked);
	assert_int_equal(locks, unlocks);
	assert_int_equal(calls, 0);
	/* Same allocator/free as obs_source_destroy's audio callback list. */
	da_free(source.audio_cb_list);
	return 0;
}

static void operation(bool add, obs_source_audio_capture_t callback, void *param)
{
	struct obs_source before;
	memcpy(&before, &source, sizeof(source));
	unsigned previous_locks = locks;
	before_count = source.audio_cb_list.num;
	if (add) {
		expected[expected_count++] = (struct audio_cb_info){callback, param};
	} else {
		for (size_t i = 0; i < expected_count; i++) {
			if (expected[i].callback == callback && expected[i].param == param) {
				memmove(&expected[i], &expected[i + 1], (expected_count - i - 1) * sizeof(expected[0]));
				expected_count--;
				break;
			}
		}
	}
	if (add)
		obs_source_add_audio_capture_callback(&source, callback, param);
	else
		obs_source_remove_audio_capture_callback(&source, callback, param);
	assert_int_equal(locks, previous_locks + 1);
	assert_int_equal(unlocks, locks);
	assert_false(locked);
	assert_int_equal(calls, 0);
	check_list();
	/* Only the C-owned darray metadata and storage may change. */
	memcpy(&before.audio_cb_list, &source.audio_cb_list, sizeof(source.audio_cb_list));
	assert_memory_equal(&source, &before, sizeof(source));
}

static void invalid_source(void **state)
{
	(void)state;
	log_handler_t old;
	void *old_param;
	base_get_log_handler(&old, &old_param);
	base_set_log_handler(capture_log, &source);
	obs_source_add_audio_capture_callback(NULL, callback_a, &tokens[0]);
	char add_message[sizeof(message)];
	memcpy(add_message, message, sizeof(message));
	obs_source_remove_audio_capture_callback(NULL, callback_b, &tokens[1]);
	base_set_log_handler(old, old_param);
	assert_int_equal(logs, 2);
	assert_string_equal(add_message, "obs_source_add_audio_capture_callback: Null 'source' parameter");
	assert_string_equal(message, "obs_source_remove_audio_capture_callback: Null 'source' parameter");
	assert_int_equal(locks, 0);
	assert_int_equal(unlocks, 0);
	check_list();
}

static void pair_identity_order_and_removal(void **state)
{
	(void)state;
	operation(false, callback_a, &tokens[0]); /* empty, still locks */
	operation(true, callback_a, &tokens[0]);
	operation(true, callback_b, &tokens[0]); /* same param, different callback */
	operation(true, callback_a, &tokens[1]); /* same callback, different param */
	operation(true, callback_a, &tokens[0]); /* duplicates append unchanged */
	operation(true, callback_b, &tokens[2]);
	operation(false, callback_a, &tokens[2]); /* neither partial match suffices */
	operation(false, callback_a, &tokens[0]); /* first match, preserve order */
	operation(false, callback_a, &tokens[0]); /* remove remaining duplicate */
	operation(false, callback_a, &tokens[0]); /* repeated absence */
	operation(false, callback_a, &tokens[1]);
	operation(false, callback_b, &tokens[2]);
	operation(false, callback_b, &tokens[0]);
	assert_int_equal(expected_count, 0);
}

static void null_pairs_are_stored_without_invocation(void **state)
{
	(void)state;
	/* Characterized, not endorsed: callback and param are not validated. */
	operation(true, NULL, NULL);
	operation(true, callback_a, NULL);
	operation(true, NULL, &tokens[0]);
	operation(true, NULL, NULL);
	operation(false, NULL, &tokens[1]);
	operation(false, NULL, NULL);
	operation(false, callback_a, NULL);
	operation(false, NULL, &tokens[0]);
	operation(false, NULL, NULL);
	operation(false, NULL, NULL);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(invalid_source, setup, teardown),
		cmocka_unit_test_setup_teardown(pair_identity_order_and_removal, setup, teardown),
		cmocka_unit_test_setup_teardown(null_pairs_are_stored_without_invocation, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
