/* Characterize the actual public C setters before the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct observation {
	obs_source_t *source;
	uint64_t expected;
	unsigned calls;
	bool talk;
	bool nested;
	int lock_result;
};

static void *probe_lock(void *param)
{
	struct observation *seen = param;
	seen->lock_result = pthread_mutex_trylock(&seen->source->audio_mutex);
	if (seen->lock_result == 0)
		pthread_mutex_unlock(&seen->source->audio_mutex);
	return NULL;
}

static void changed(void *param, calldata_t *data)
{
	struct observation *seen = param;
	void *source = NULL;
	long long delay = 0;
	assert_true(calldata_get_ptr(data, "source", &source));
	assert_true(calldata_get_int(data, "delay", &delay));
	assert_ptr_equal(source, seen->source);
	/* Characterized, not endorsed: the signal casts uint64_t to signed. */
	assert_true((uint64_t)delay == seen->expected);
	assert_true((seen->talk ? seen->source->push_to_talk_delay : seen->source->push_to_mute_delay) ==
		    seen->expected);
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, NULL, probe_lock, seen), 0);
	assert_int_equal(pthread_join(thread, NULL), 0);
	assert_int_equal(seen->lock_result, EBUSY);
	/* Delivery is synchronous under a nonrecursive mutex. Reentering a
	 * setter/getter on this source would deadlock. A different source is safe. */
	if (!seen->nested) {
		struct obs_source other = {0};
		assert_int_equal(pthread_mutex_init(&other.audio_mutex, NULL), 0);
		other.context.signals = signal_handler_create();
		assert_non_null(other.context.signals);
		assert_true(
			signal_handler_add(other.context.signals, "void push_to_talk_delay(ptr source, int delay)"));
		struct observation nested = {.source = &other, .expected = UINT64_MAX, .talk = true, .nested = true};
		signal_handler_connect(other.context.signals, "push_to_talk_delay", changed, &nested);
		obs_source_set_push_to_talk_delay(&other, UINT64_MAX);
		assert_int_equal(nested.calls, 1);
		signal_handler_destroy(other.context.signals);
		assert_int_equal(pthread_mutex_destroy(&other.audio_mutex), 0);
	}
	/* Callback edits to calldata do not write the stored delay back. */
	calldata_set_int(data, "delay", 17);
	seen->calls++;
}

static void delays(void **state)
{
	(void)state;
	struct obs_source source = {0};
	assert_int_equal(pthread_mutex_init(&source.audio_mutex, NULL), 0);
	source.context.signals = signal_handler_create();
	assert_non_null(source.context.signals);
	assert_true(signal_handler_add(source.context.signals, "void push_to_mute_delay(ptr source, int delay)"));
	assert_true(signal_handler_add(source.context.signals, "void push_to_talk_delay(ptr source, int delay)"));
	struct observation mute = {.source = &source};
	struct observation talk = {.source = &source, .talk = true};
	signal_handler_connect(source.context.signals, "push_to_mute_delay", changed, &mute);
	signal_handler_connect(source.context.signals, "push_to_talk_delay", changed, &talk);
	const uint64_t values[] = {
		0,          0,          1, UINT32_MAX, (uint64_t)UINT32_MAX + 1, INT64_MAX, (uint64_t)INT64_MAX + 1,
		UINT64_MAX, UINT64_MAX, 0};
	uint64_t previous = 0;
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		mute.expected = talk.expected = values[i];
		obs_source_set_push_to_mute_delay(&source, values[i]);
		assert_int_equal(mute.calls, i + 1);
		assert_int_equal(talk.calls, i);
		assert_true(obs_source_get_push_to_mute_delay(&source) == values[i]);
		assert_true(obs_source_get_push_to_talk_delay(&source) == previous);
		obs_source_set_push_to_talk_delay(&source, values[i]);
		assert_int_equal(talk.calls, i + 1);
		assert_int_equal(mute.calls, i + 1);
		assert_true(obs_source_get_push_to_talk_delay(&source) == values[i]);
		assert_true(obs_source_get_push_to_mute_delay(&source) == values[i]);
		/* Both setters have released the mutex on return. */
		assert_int_equal(pthread_mutex_trylock(&source.audio_mutex), 0);
		assert_int_equal(pthread_mutex_unlock(&source.audio_mutex), 0);
		previous = values[i];
	}
	signal_handler_destroy(source.context.signals);
	assert_int_equal(pthread_mutex_destroy(&source.audio_mutex), 0);
}

static unsigned log_count;
static char log_message[128];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	assert_int_equal(level, LOG_DEBUG);
	log_count++;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void null_source(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	char messages[4][128];
	for (unsigned i = 0; i < 2; i++) {
		obs_source_set_push_to_mute_delay(NULL, i ? UINT64_MAX : 0);
		memcpy(messages[i * 2], log_message, sizeof(log_message));
		obs_source_set_push_to_talk_delay(NULL, i ? UINT64_MAX : 0);
		memcpy(messages[i * 2 + 1], log_message, sizeof(log_message));
	}
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(log_count, 4);
	for (unsigned i = 0; i < 2; i++) {
		assert_string_equal(messages[i * 2], "obs_source_set_push_to_mute_delay: Null 'source' parameter");
		assert_string_equal(messages[i * 2 + 1], "obs_source_set_push_to_talk_delay: Null 'source' parameter");
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(delays), cmocka_unit_test(null_source)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
