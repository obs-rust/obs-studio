#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>

#include <cmocka.h>

#include "obs-preflight.h"

_Static_assert(OBS_PREFLIGHT_PASS == 0, "Pass verdict ABI value");
_Static_assert(OBS_PREFLIGHT_MARGINAL == 1, "Marginal verdict ABI value");
_Static_assert(OBS_PREFLIGHT_FAIL == 2, "Fail verdict ABI value");

static struct obs_preflight_sample sample_at(uint64_t timestamp_ms)
{
	return (struct obs_preflight_sample){
		.timestamp_ms = timestamp_ms,
		.average_frame_time_ns = 8000000,
		.frame_interval_ns = 16666667,
		.output_congestion = 0.0,
		.cpu_percent = 50.0,
	};
}

static void push(obs_preflight_engine_t *engine, struct obs_preflight_sample sample)
{
	assert_int_equal(obs_preflight_engine_push_sample(engine, &sample), OBS_PREFLIGHT_OK);
}

static int32_t run_output_drop_case(uint64_t dropped_frames)
{
	obs_preflight_engine_t *engine = obs_preflight_engine_create_with_window(1000);
	struct obs_preflight_sample first = sample_at(0);
	struct obs_preflight_sample second = sample_at(1000);
	struct obs_preflight_window window;

	assert_non_null(engine);
	second.output_total_bytes = 500000;
	second.output_total_frames = 10000;
	second.output_frames_dropped = dropped_frames;
	second.encoder_total_frames = 10000;
	push(engine, first);
	push(engine, second);
	assert_int_equal(obs_preflight_engine_get_window(engine, 0, &window), OBS_PREFLIGHT_OK);
	obs_preflight_engine_destroy(engine);
	return window.verdict;
}

static void test_create_zero_window_and_destroy_null(void **state)
{
	(void)state;
	assert_null(obs_preflight_engine_create_with_window(0));
	obs_preflight_engine_destroy(NULL);
}

static void test_null_arguments_return_invalid_argument(void **state)
{
	(void)state;
	struct obs_preflight_sample sample = sample_at(0);
	struct obs_preflight_window window;
	size_t count = 0;
	obs_preflight_engine_t *engine = obs_preflight_engine_create();

	assert_non_null(engine);
	assert_int_equal(obs_preflight_engine_push_sample(NULL, &sample), OBS_PREFLIGHT_INVALID_ARGUMENT);
	assert_int_equal(obs_preflight_engine_push_sample(engine, NULL), OBS_PREFLIGHT_INVALID_ARGUMENT);
	assert_int_equal(obs_preflight_engine_get_window_count(NULL, &count), OBS_PREFLIGHT_INVALID_ARGUMENT);
	assert_int_equal(obs_preflight_engine_get_window_count(engine, NULL), OBS_PREFLIGHT_INVALID_ARGUMENT);
	assert_int_equal(obs_preflight_engine_get_window(NULL, 0, &window), OBS_PREFLIGHT_INVALID_ARGUMENT);
	assert_int_equal(obs_preflight_engine_get_window(engine, 0, NULL), OBS_PREFLIGHT_INVALID_ARGUMENT);
	obs_preflight_engine_destroy(engine);
}

static void test_window_queries_need_two_samples(void **state)
{
	(void)state;
	obs_preflight_engine_t *engine = obs_preflight_engine_create_with_window(1000);
	struct obs_preflight_sample sample = sample_at(0);
	struct obs_preflight_window window;
	size_t count = 0;

	assert_non_null(engine);
	assert_int_equal(obs_preflight_engine_get_window_count(engine, &count), OBS_PREFLIGHT_ANALYSIS_ERROR);
	assert_int_equal(obs_preflight_engine_get_window(engine, 0, &window), OBS_PREFLIGHT_ANALYSIS_ERROR);
	push(engine, sample);
	assert_int_equal(obs_preflight_engine_get_window_count(engine, &count), OBS_PREFLIGHT_ANALYSIS_ERROR);
	assert_int_equal(obs_preflight_engine_get_window(engine, 0, &window), OBS_PREFLIGHT_ANALYSIS_ERROR);
	obs_preflight_engine_destroy(engine);
}

static void test_clean_window_passes_with_reason(void **state)
{
	(void)state;
	obs_preflight_engine_t *engine = obs_preflight_engine_create_with_window(1000);
	struct obs_preflight_sample first = sample_at(0);
	struct obs_preflight_sample second = sample_at(1000);
	struct obs_preflight_window window;
	size_t count = 0;

	assert_non_null(engine);
	second.output_total_bytes = 500000;
	second.output_total_frames = 10000;
	second.encoder_total_frames = 10000;
	push(engine, first);
	push(engine, second);
	assert_int_equal(obs_preflight_engine_get_window_count(engine, &count), OBS_PREFLIGHT_OK);
	assert_int_equal(count, 1);
	assert_int_equal(obs_preflight_engine_get_window(engine, 0, &window), OBS_PREFLIGHT_OK);
	assert_int_equal(window.verdict, OBS_PREFLIGHT_PASS);
	assert_true(window.reason[0] != '\0');
	assert_non_null(memchr(window.reason, '\0', sizeof(window.reason)));
	obs_preflight_engine_destroy(engine);
}

static void test_output_drop_thresholds(void **state)
{
	(void)state;
	assert_int_equal(run_output_drop_case(100), OBS_PREFLIGHT_PASS);
	assert_int_equal(run_output_drop_case(501), OBS_PREFLIGHT_FAIL);
}

static void test_counter_decrease_starts_a_new_window(void **state)
{
	(void)state;
	obs_preflight_engine_t *engine = obs_preflight_engine_create_with_window(5000);
	struct obs_preflight_sample samples[4];
	size_t count = 0;

	assert_non_null(engine);
	for (size_t i = 0; i < 4; ++i)
		samples[i] = sample_at((uint64_t)i * 1000);
	samples[0].output_total_bytes = 10;
	samples[1].output_total_bytes = 20;
	samples[2].output_total_bytes = 0;
	samples[3].output_total_bytes = 5;
	for (size_t i = 0; i < 4; ++i) {
		samples[i].output_total_frames = (uint64_t)i * 10;
		samples[i].output_frames_dropped = (uint64_t)i;
		samples[i].encoder_total_frames = (uint64_t)i * 10;
		push(engine, samples[i]);
	}
	assert_int_equal(obs_preflight_engine_get_window_count(engine, &count), OBS_PREFLIGHT_OK);
	assert_int_equal(count, 2);
	obs_preflight_engine_destroy(engine);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_create_zero_window_and_destroy_null),
		cmocka_unit_test(test_null_arguments_return_invalid_argument),
		cmocka_unit_test(test_window_queries_need_two_samples),
		cmocka_unit_test(test_clean_window_passes_with_reason),
		cmocka_unit_test(test_output_drop_thresholds),
		cmocka_unit_test(test_counter_decrease_starts_a_new_window),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
