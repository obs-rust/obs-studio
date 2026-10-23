/* Regression for the uninitialized interval in add_audio_buffering (#838).
 * Compile the original private implementation directly: this checks the C
 * buffering algorithm, rather than the Rust swap or real-time scheduling.
 * Unused core functions are discarded at link time. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include "libobs/obs-audio-mix.c.h"
#include "libobs/obs-audio.c.h"

static const uint64_t start = 1000000000ULL;

static void subsample_delay_preserves_interval(void **state)
{
	(void)state;
	const size_t rates[] = {44100, 48000, 96000};
	for (size_t i = 0; i < sizeof(rates) / sizeof(rates[0]); i++) {
		const uint64_t offsets[] = {1, 10000, (1000000000ULL - 1) / rates[i]};
		for (size_t j = 0; j < sizeof(offsets) / sizeof(offsets[0]); j++) {
			struct obs_core_audio audio = {.max_buffering_ticks = 45};
			const struct ts_info original = {start,
							 start + audio_frames_to_ns(rates[i], AUDIO_OUTPUT_FRAMES)};
			struct ts_info ts = original;
			assert_int_equal(ns_to_audio_frames(rates[i], offsets[j]), 0);
			add_audio_buffering(&audio, rates[i], &ts, start - offsets[j], "subsample");
			assert_int_equal(ts.start, original.start);
			assert_int_equal(ts.end, original.end);
			assert_int_equal(audio.total_buffering_ticks, 0);
			assert_int_equal(audio.buffering_wait_ticks, 0);
			assert_int_equal(audio.buffered_timestamps.size, 0);
			deque_free(&audio.buffered_timestamps);
		}
	}
}

static void subsample_delay_preserves_pending_interval(void **state)
{
	(void)state;
	const uint64_t tick = audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	struct obs_core_audio audio = {.max_buffering_ticks = 45,
				       .total_buffering_ticks = 1,
				       .buffering_wait_ticks = 1,
				       .buffered_ts = start + tick};
	const struct ts_info original = {start, start + tick};
	struct ts_info ts = original;
	deque_push_front(&audio.buffered_timestamps, &original, sizeof(original));
	add_audio_buffering(&audio, 48000, &ts, start - 1, "pending");
	struct ts_info queued;
	deque_peek_front(&audio.buffered_timestamps, &queued, sizeof(queued));
	deque_free(&audio.buffered_timestamps);
	assert_int_equal(ts.start, original.start);
	assert_int_equal(ts.end, original.end);
	assert_int_equal(queued.start, original.start);
	assert_int_equal(queued.end, original.end);
	assert_int_equal(audio.total_buffering_ticks, 1);
	assert_int_equal(audio.buffering_wait_ticks, 1);
}

static void first_sample_adds_one_tick(void **state)
{
	(void)state;
	struct obs_core_audio audio = {.max_buffering_ticks = 45};
	struct ts_info ts = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	assert_int_equal(ns_to_audio_frames(48000, 20834), 1);
	add_audio_buffering(&audio, 48000, &ts, start - 20834, "one sample");
	struct ts_info queued;
	deque_peek_front(&audio.buffered_timestamps, &queued, sizeof(queued));
	deque_free(&audio.buffered_timestamps);
	assert_int_equal(audio.total_buffering_ticks, 1);
	assert_int_equal(audio.buffering_wait_ticks, 1);
	assert_int_equal(ts.start, start - audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES));
	assert_int_equal(ts.end, start);
	assert_int_equal(queued.start, ts.start);
	assert_int_equal(queued.end, ts.end);
}

static void larger_delay_adds_two_ticks(void **state)
{
	(void)state;
	struct obs_core_audio audio = {.max_buffering_ticks = 45};
	struct ts_info ts = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	add_audio_buffering(&audio, 48000, &ts, start - 22000000, "two ticks");
	struct ts_info queued;
	deque_peek_front(&audio.buffered_timestamps, &queued, sizeof(queued));
	assert_int_equal(audio.buffered_timestamps.size, 2 * sizeof(queued));
	deque_free(&audio.buffered_timestamps);
	assert_int_equal(audio.total_buffering_ticks, 2);
	assert_int_equal(audio.buffering_wait_ticks, 2);
	assert_int_equal(ts.start, start - audio_frames_to_ns(48000, 2 * AUDIO_OUTPUT_FRAMES));
	assert_int_equal(ts.end, start - audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES));
	assert_int_equal(queued.start, ts.start);
	assert_int_equal(queued.end, ts.end);
}

static void full_buffer_preserves_interval(void **state)
{
	(void)state;
	struct obs_core_audio audio = {.max_buffering_ticks = 45, .total_buffering_ticks = 45};
	const struct ts_info original = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	struct ts_info ts = original;
	add_audio_buffering(&audio, 48000, &ts, start - 1, "full");
	assert_int_equal(ts.start, original.start);
	assert_int_equal(ts.end, original.end);
	assert_int_equal(audio.total_buffering_ticks, 45);
	assert_int_equal(audio.buffering_wait_ticks, 0);
	assert_int_equal(audio.buffered_timestamps.size, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(subsample_delay_preserves_interval),
		cmocka_unit_test(subsample_delay_preserves_pending_interval),
		cmocka_unit_test(first_sample_adds_one_tick),
		cmocka_unit_test(larger_delay_adds_two_ticks),
		cmocka_unit_test(full_buffer_preserves_interval),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
