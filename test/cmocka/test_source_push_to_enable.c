/* Characterize the public enable setters against the actual library. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct observation {
	obs_source_t *source;
	unsigned calls;
	bool enabled;
	bool talk;
};

static void changed(void *param, calldata_t *data)
{
	struct observation *seen = param;
	void *source = NULL;
	bool enabled = false;
	assert_true(calldata_get_ptr(data, "source", &source));
	assert_true(calldata_get_bool(data, "enabled", &enabled));
	assert_ptr_equal(source, seen->source);
	assert_int_equal(enabled, seen->enabled);
	/* The store precedes the synchronous signal, while audio_mutex is held.
	 * Read the field here; the public getters lock that same mutex. */
	assert_int_equal(seen->talk ? seen->source->push_to_talk_enabled : seen->source->push_to_mute_enabled,
			 enabled);
	seen->calls++;
}

static void transitions(void **state)
{
	(void)state;
	struct obs_source source = {0};
	assert_int_equal(pthread_mutex_init(&source.audio_mutex, NULL), 0);
	source.context.signals = signal_handler_create();
	assert_non_null(source.context.signals);
	assert_true(signal_handler_add(source.context.signals, "void push_to_mute_changed(ptr source, bool enabled)"));
	assert_true(signal_handler_add(source.context.signals, "void push_to_talk_changed(ptr source, bool enabled)"));
	struct observation mute = {.source = &source};
	struct observation talk = {.source = &source, .talk = true};
	signal_handler_connect(source.context.signals, "push_to_mute_changed", changed, &mute);
	signal_handler_connect(source.context.signals, "push_to_talk_changed", changed, &talk);

	/* Initial false and repeated true/false values do not signal. */
	const bool values[] = {false, true, true, false, false, true};
	unsigned expected = 0;
	bool previous = false;
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		const bool value = values[i];
		expected += value != previous;
		mute.enabled = value;
		obs_source_enable_push_to_mute(&source, value);
		assert_int_equal(mute.calls, expected);
		assert_int_equal(talk.calls, expected - (value != previous));
		assert_int_equal(obs_source_push_to_mute_enabled(&source), value);
		assert_int_equal(obs_source_push_to_talk_enabled(&source), previous);

		talk.enabled = value;
		obs_source_enable_push_to_talk(&source, value);
		assert_int_equal(talk.calls, expected);
		assert_int_equal(mute.calls, expected);
		assert_int_equal(obs_source_push_to_talk_enabled(&source), value);
		assert_int_equal(obs_source_push_to_mute_enabled(&source), value);
		previous = value;
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
		obs_source_enable_push_to_mute(NULL, i != 0);
		memcpy(messages[i * 2], log_message, sizeof(log_message));
		obs_source_enable_push_to_talk(NULL, i != 0);
		memcpy(messages[i * 2 + 1], log_message, sizeof(log_message));
	}
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(log_count, 4);
	for (unsigned i = 0; i < 2; i++) {
		assert_string_equal(messages[i * 2], "obs_source_enable_push_to_mute: Null 'source' parameter");
		assert_string_equal(messages[i * 2 + 1], "obs_source_enable_push_to_talk: Null 'source' parameter");
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(transitions), cmocka_unit_test(null_source)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
