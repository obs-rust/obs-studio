/* Freeze both audio-pause subscriptions through the real libobs exports. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

static void count_callback(void *param, calldata_t *data)
{
	assert_non_null(data);
	++*(unsigned *)param;
}

static void subscriptions_and_removal(void **state)
{
	(void)state;
	struct obs_source source = {0};
	signal_handler_t *handler = signal_handler_create();
	assert_non_null(handler);
	source.context.signals = handler;
	assert_true(signal_handler_add(handler, "void media_pause()"));
	assert_true(signal_handler_add(handler, "void media_stopped()"));
	calldata_t data;
	calldata_init(&data);
	unsigned calls = 0, other_calls = 0;

	obs_source_add_audio_pause_callback(&source, count_callback, &calls);
	obs_source_add_audio_pause_callback(&source, count_callback, &other_calls);
	signal_handler_signal(handler, "media_pause", &data);
	assert_int_equal(calls, 1);
	assert_int_equal(other_calls, 1);
	signal_handler_signal(handler, "media_stopped", &data);
	assert_int_equal(calls, 2);
	assert_int_equal(other_calls, 2);

	obs_source_remove_audio_pause_callback(&source, count_callback, &calls);
	signal_handler_signal(handler, "media_pause", &data);
	assert_int_equal(calls, 2);
	assert_int_equal(other_calls, 3);
	signal_handler_signal(handler, "media_stopped", &data);
	assert_int_equal(calls, 2);
	assert_int_equal(other_calls, 4);
	obs_source_remove_audio_pause_callback(&source, count_callback, &other_calls);
	calldata_free(&data);
	signal_handler_destroy(handler);
}

static void null_source(void **state)
{
	(void)state;
	unsigned calls = 0;
	obs_source_add_audio_pause_callback(NULL, count_callback, &calls);
	obs_source_remove_audio_pause_callback(NULL, count_callback, &calls);
	assert_int_equal(calls, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(subscriptions_and_removal), cmocka_unit_test(null_source)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
