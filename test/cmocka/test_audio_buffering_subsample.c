/* Regression for the uninitialized interval in add_audio_buffering (#838),
 * and characterization of the rest of obs-audio-mix.c (time to frames, the
 * mix window and add loop, the fixed and added buffering steps) before its
 * port to obs-core (#1903).
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

/* Characterization of obs-audio-mix.c before its port to obs-core (#1903). */

/* floor(ns * rate / 1e9), independent of util_mul_div64. */
static size_t frames_at(size_t rate, uint64_t ns)
{
	return (size_t)(((unsigned __int128)ns * rate) / 1000000000ULL);
}

static void time_to_frames_rounds_down(void **state)
{
	(void)state;
	const size_t rates[] = {8000, 22050, 44100, 48000, 96000, 192000};
	for (size_t i = 0; i < sizeof(rates) / sizeof(rates[0]); i++) {
		for (uint64_t frames = 0; frames <= AUDIO_OUTPUT_FRAMES; frames++) {
			const uint64_t ns = audio_frames_to_ns(rates[i], frames);
			assert_int_equal(convert_time_to_frames(rates[i], ns), frames_at(rates[i], ns));
			assert_int_equal(convert_time_to_frames(rates[i], ns + 1), frames_at(rates[i], ns + 1));
		}
	}
	/* audio_frames_to_ns rounds down too, so a frame's own time can map to
	 * the frame before it: characterized, not endorsed. */
	assert_int_equal(convert_time_to_frames(44100, audio_frames_to_ns(44100, 1)), 0);
	assert_int_equal(convert_time_to_frames(48000, audio_frames_to_ns(48000, 3)), 3);
}

#define MIX_CHANNELS 2

static float mix_out[MAX_AUDIO_MIXES][MAX_AUDIO_CHANNELS][AUDIO_OUTPUT_FRAMES];
static float source_out[MAX_AUDIO_MIXES][MAX_AUDIO_CHANNELS][AUDIO_OUTPUT_FRAMES];

/* Every output sample starts at its own value, every source sample is
 * 1000 + its position, so a sum shows which source frame went where. */
static void mix_setup(struct audio_output_data *mixes, struct obs_source *source)
{
	memset(source, 0, sizeof(*source));
	for (size_t m = 0; m < MAX_AUDIO_MIXES; m++) {
		for (size_t c = 0; c < MAX_AUDIO_CHANNELS; c++) {
			mixes[m].data[c] = mix_out[m][c];
			source->audio_output_buf[m][c] = source_out[m][c];
			for (size_t f = 0; f < AUDIO_OUTPUT_FRAMES; f++) {
				mix_out[m][c][f] = (float)((m * 16 + c) * 2048 + f);
				source_out[m][c][f] = (float)(1000 + f);
			}
		}
	}
}

/* With the source's audio from frame first on: frames before first unchanged,
 * frame f gets source frame f - first, channels past MIX_CHANNELS unchanged. */
static void assert_mixed_from(size_t first)
{
	for (size_t m = 0; m < MAX_AUDIO_MIXES; m++) {
		for (size_t c = 0; c < MAX_AUDIO_CHANNELS; c++) {
			for (size_t f = 0; f < AUDIO_OUTPUT_FRAMES; f++) {
				float expected = (float)((m * 16 + c) * 2048 + f);
				if (c < MIX_CHANNELS && f >= first)
					expected += (float)(1000 + f - first);
				assert_true(mix_out[m][c][f] == expected);
			}
		}
	}
}

static struct obs_source mix_source;

static void mix_whole_tick(void **state)
{
	(void)state;
	struct audio_output_data mixes[MAX_AUDIO_MIXES];
	struct ts_info ts = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	mix_setup(mixes, &mix_source);
	mix_source.audio_ts = ts.start;
	mix_audio(mixes, &mix_source, MIX_CHANNELS, 48000, &ts);
	assert_mixed_from(0);
}

static void mix_from_start_point(void **state)
{
	(void)state;
	const size_t rates[] = {44100, 48000};
	const size_t frames[] = {1, 2, 511, 512, 1023};
	for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++) {
		for (size_t i = 0; i < sizeof(frames) / sizeof(frames[0]); i++) {
			struct audio_output_data mixes[MAX_AUDIO_MIXES];
			struct ts_info ts = {start, start + audio_frames_to_ns(rates[r], AUDIO_OUTPUT_FRAMES)};
			mix_setup(mixes, &mix_source);
			mix_source.audio_ts = start + audio_frames_to_ns(rates[r], frames[i]);
			mix_audio(mixes, &mix_source, MIX_CHANNELS, rates[r], &ts);
			assert_mixed_from(frames_at(rates[r], mix_source.audio_ts - start));
		}
	}
}

static void mix_outside_tick_adds_nothing(void **state)
{
	(void)state;
	const struct ts_info window = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	const uint64_t times[] = {start - 1, window.end, window.end + 1};
	for (size_t i = 0; i < sizeof(times) / sizeof(times[0]); i++) {
		struct audio_output_data mixes[MAX_AUDIO_MIXES];
		struct ts_info ts = window;
		mix_setup(mixes, &mix_source);
		mix_source.audio_ts = times[i];
		mix_audio(mixes, &mix_source, MIX_CHANNELS, 48000, &ts);
		assert_mixed_from(AUDIO_OUTPUT_FRAMES);
		assert_int_equal(ts.start, window.start);
		assert_int_equal(ts.end, window.end);
	}
}

/* A window longer than one tick: audio on frame 1024 is inside the window
 * but adds nothing. */
static void mix_on_frame_1024_adds_nothing(void **state)
{
	(void)state;
	struct audio_output_data mixes[MAX_AUDIO_MIXES];
	struct ts_info ts = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES + 1)};
	mix_setup(mixes, &mix_source);
	mix_source.audio_ts = start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES) + 1;
	assert_int_equal(frames_at(48000, mix_source.audio_ts - start), AUDIO_OUTPUT_FRAMES);
	mix_audio(mixes, &mix_source, MIX_CHANNELS, 48000, &ts);
	assert_mixed_from(AUDIO_OUTPUT_FRAMES);
}

static void buffering_maxed_only_at_max(void **state)
{
	(void)state;
	struct obs_core_audio audio = {.max_buffering_ticks = 45};
	assert_false(audio_buffering_maxed(&audio));
	audio.total_buffering_ticks = 44;
	assert_false(audio_buffering_maxed(&audio));
	audio.total_buffering_ticks = 45;
	assert_true(audio_buffering_maxed(&audio));
	/* Only equality counts: characterized, not endorsed. */
	audio.total_buffering_ticks = 46;
	assert_false(audio_buffering_maxed(&audio));
}

/* The queue holds count contiguous one-tick windows before buffered_ts,
 * earliest at the front: the front one starts first + count ticks before
 * buffered_ts, the back one ends first ticks before it. Pops them all. */
static void assert_queued_ticks(struct obs_core_audio *audio, size_t rate, uint64_t buffered_ts, uint64_t first,
				uint64_t count)
{
	assert_int_equal(audio->buffered_timestamps.size, count * sizeof(struct ts_info));
	for (uint64_t i = 0; i < count; i++) {
		struct ts_info queued;
		deque_pop_front(&audio->buffered_timestamps, &queued, sizeof(queued));
		const uint64_t ticks = first + count - i;
		assert_int_equal(queued.start, buffered_ts - audio_frames_to_ns(rate, ticks * AUDIO_OUTPUT_FRAMES));
		assert_int_equal(queued.end, buffered_ts - audio_frames_to_ns(rate, (ticks - 1) * AUDIO_OUTPUT_FRAMES));
	}
}

static void fixed_buffering_fills_to_max(void **state)
{
	(void)state;
	struct obs_core_audio audio = {.max_buffering_ticks = 45};
	struct ts_info ts = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	set_fixed_audio_buffering(&audio, 48000, &ts);
	assert_int_equal(audio.total_buffering_ticks, 45);
	assert_int_equal(audio.buffering_wait_ticks, 45);
	assert_int_equal(audio.buffered_ts, start);
	assert_int_equal(ts.start, start - audio_frames_to_ns(48000, 45 * AUDIO_OUTPUT_FRAMES));
	assert_int_equal(ts.end, start - audio_frames_to_ns(48000, 44 * AUDIO_OUTPUT_FRAMES));
	assert_queued_ticks(&audio, 48000, start, 0, 45);
	deque_free(&audio.buffered_timestamps);
}

static void fixed_buffering_keeps_buffered_ts_while_waiting(void **state)
{
	(void)state;
	const uint64_t buffered_ts = start + 5 * audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	struct obs_core_audio audio = {.max_buffering_ticks = 45,
				       .total_buffering_ticks = 3,
				       .buffering_wait_ticks = 2,
				       .buffered_ts = buffered_ts};
	struct ts_info ts = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	set_fixed_audio_buffering(&audio, 48000, &ts);
	assert_int_equal(audio.total_buffering_ticks, 45);
	assert_int_equal(audio.buffering_wait_ticks, 44);
	assert_int_equal(audio.buffered_ts, buffered_ts);
	assert_int_equal(ts.start, buffered_ts - audio_frames_to_ns(48000, 44 * AUDIO_OUTPUT_FRAMES));
	assert_queued_ticks(&audio, 48000, buffered_ts, 2, 42);
	deque_free(&audio.buffered_timestamps);
}

static void fixed_buffering_at_max_changes_nothing(void **state)
{
	(void)state;
	struct obs_core_audio audio = {.max_buffering_ticks = 45, .total_buffering_ticks = 45};
	const struct ts_info original = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	struct ts_info ts = original;
	set_fixed_audio_buffering(&audio, 48000, &ts);
	assert_int_equal(ts.start, original.start);
	assert_int_equal(ts.end, original.end);
	assert_int_equal(audio.total_buffering_ticks, 45);
	assert_int_equal(audio.buffering_wait_ticks, 0);
	assert_int_equal(audio.buffered_timestamps.size, 0);
}

static void buffering_stops_at_max(void **state)
{
	(void)state;
	const uint64_t tick = audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	struct obs_core_audio audio = {.max_buffering_ticks = 45, .total_buffering_ticks = 43};
	struct ts_info ts = {start, start + tick};
	/* 5 ticks late, room for 2. */
	add_audio_buffering(&audio, 48000, &ts, start - 5 * tick, "capped");
	assert_int_equal(audio.total_buffering_ticks, 45);
	assert_int_equal(audio.buffering_wait_ticks, 2);
	assert_int_equal(ts.start, start - audio_frames_to_ns(48000, 2 * AUDIO_OUTPUT_FRAMES));
	assert_queued_ticks(&audio, 48000, start, 0, 2);
	deque_free(&audio.buffered_timestamps);
}

static void buffering_rounds_ticks_up(void **state)
{
	(void)state;
	const size_t rates[] = {44100, 48000};
	for (size_t r = 0; r < sizeof(rates) / sizeof(rates[0]); r++) {
		const uint64_t frames[] = {1, 1023, 1024, 1025, 2048, 2049};
		for (size_t i = 0; i < sizeof(frames) / sizeof(frames[0]); i++) {
			struct obs_core_audio audio = {.max_buffering_ticks = 45};
			struct ts_info ts = {start, start + audio_frames_to_ns(rates[r], AUDIO_OUTPUT_FRAMES)};
			const uint64_t late = audio_frames_to_ns(rates[r], frames[i]) + 1;
			const uint64_t late_frames = ns_to_audio_frames(rates[r], late);
			const uint64_t ticks = (late_frames + AUDIO_OUTPUT_FRAMES - 1) / AUDIO_OUTPUT_FRAMES;
			add_audio_buffering(&audio, rates[r], &ts, start - late, "rounding");
			assert_int_equal(audio.total_buffering_ticks, ticks);
			assert_int_equal(audio.buffering_wait_ticks, ticks);
			assert_int_equal(ts.start, start - audio_frames_to_ns(rates[r], ticks * AUDIO_OUTPUT_FRAMES));
			assert_queued_ticks(&audio, rates[r], start, 0, ticks);
			deque_free(&audio.buffered_timestamps);
		}
	}
}

/* The core in obs-audio-mix.c, on plain values: no source and no
 * struct obs_core_audio. */

static void core_mix_on_plain_buffers(void **state)
{
	(void)state;
	struct audio_output_data mixes[MAX_AUDIO_MIXES];
	float *buf[MAX_AUDIO_MIXES][MAX_AUDIO_CHANNELS];
	struct ts_info ts = {start, start + audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES)};
	mix_setup(mixes, &mix_source);
	memcpy(buf, mix_source.audio_output_buf, sizeof(buf));

	/* Frame 512's time maps to frame 511 at 48 kHz (both round down). */
	obs_audio_mix_add(mixes, buf, start + audio_frames_to_ns(48000, 512), &ts, MIX_CHANNELS, 48000);
	assert_mixed_from(frames_at(48000, audio_frames_to_ns(48000, 512)));
	assert_int_equal(frames_at(48000, audio_frames_to_ns(48000, 512)), 511);

	/* Outside the tick: nothing. */
	mix_setup(mixes, &mix_source);
	obs_audio_mix_add(mixes, buf, ts.end, &ts, MIX_CHANNELS, 48000);
	assert_mixed_from(AUDIO_OUTPUT_FRAMES);
	assert_int_equal(obs_audio_mix_time_to_frames(44100, audio_frames_to_ns(44100, 1)), 0);
}

struct core_buffering {
	uint64_t buffered_ts;
	struct deque buffered_timestamps;
	uint64_t buffering_wait_ticks;
	int total_buffering_ticks;
	struct obs_audio_buffering fields;
};

static void core_buffering_init(struct core_buffering *b, int total, int max)
{
	memset(b, 0, sizeof(*b));
	b->total_buffering_ticks = total;
	b->fields.buffered_ts = &b->buffered_ts;
	b->fields.buffered_timestamps = &b->buffered_timestamps;
	b->fields.buffering_wait_ticks = &b->buffering_wait_ticks;
	b->fields.total_buffering_ticks = &b->total_buffering_ticks;
	b->fields.max_buffering_ticks = max;
}

static void core_buffering_steps(void **state)
{
	(void)state;
	const uint64_t tick = audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	struct core_buffering b;
	struct obs_audio_buffering_step step;
	struct ts_info ts;

	/* At the maximum: nothing, and added is false. */
	core_buffering_init(&b, 45, 45);
	ts = (struct ts_info){start, start + tick};
	assert_true(obs_audio_buffering_maxed(&b.fields));
	step = obs_audio_buffering_fixed(&b.fields, 48000, &ts);
	assert_false(step.added);
	step = obs_audio_buffering_add(&b.fields, 48000, &ts, start - tick);
	assert_false(step.added);
	assert_int_equal(ts.start, start);
	assert_int_equal(b.buffered_timestamps.size, 0);

	/* Fixed from 0: 45 ticks, 960 ms at 48 kHz. */
	core_buffering_init(&b, 0, 45);
	ts = (struct ts_info){start, start + tick};
	step = obs_audio_buffering_fixed(&b.fields, 48000, &ts);
	assert_true(step.added);
	assert_false(step.max_reached);
	assert_int_equal(step.ticks, 45);
	assert_int_equal(step.ms, 960);
	assert_int_equal(step.total_ms, 960);
	assert_int_equal(b.total_buffering_ticks, 45);
	assert_int_equal(b.buffering_wait_ticks, 45);
	assert_int_equal(b.buffered_ts, start);
	assert_int_equal(b.buffered_timestamps.size, 45 * sizeof(struct ts_info));
	assert_int_equal(ts.start, start - audio_frames_to_ns(48000, 45 * AUDIO_OUTPUT_FRAMES));
	deque_free(&b.buffered_timestamps);

	/* 5 ticks late with room for 2: cut at the maximum. */
	core_buffering_init(&b, 43, 45);
	ts = (struct ts_info){start, start + tick};
	step = obs_audio_buffering_add(&b.fields, 48000, &ts, start - 5 * tick);
	assert_true(step.added);
	assert_true(step.max_reached);
	assert_int_equal(step.ticks, 2);
	assert_int_equal(step.ms, 2 * 1024 * 1000 / 48000);
	assert_int_equal(step.total_ms, 960);
	assert_int_equal(b.buffering_wait_ticks, 2);
	deque_free(&b.buffered_timestamps);

	/* One sample late at 44.1 kHz: one tick, 23 ms. */
	core_buffering_init(&b, 0, 45);
	ts = (struct ts_info){start, start + audio_frames_to_ns(44100, AUDIO_OUTPUT_FRAMES)};
	step = obs_audio_buffering_add(&b.fields, 44100, &ts, start - audio_frames_to_ns(44100, 1) - 1);
	assert_true(step.added);
	assert_false(step.max_reached);
	assert_int_equal(step.ticks, 1);
	assert_int_equal(step.ms, 1024 * 1000 / 44100);
	assert_int_equal(step.total_ms, 1024 * 1000 / 44100);
	deque_free(&b.buffered_timestamps);

	/* Sub-sample late: added with 0 ticks and 0 ms, ts unchanged. */
	core_buffering_init(&b, 0, 45);
	ts = (struct ts_info){start, start + tick};
	step = obs_audio_buffering_add(&b.fields, 48000, &ts, start - 1);
	assert_true(step.added);
	assert_int_equal(step.ticks, 0);
	assert_int_equal(step.ms, 0);
	assert_int_equal(step.total_ms, 0);
	assert_int_equal(ts.start, start);
	assert_int_equal(ts.end, start + tick);
	assert_int_equal(b.buffered_ts, start);
	assert_int_equal(b.buffered_timestamps.size, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(subsample_delay_preserves_interval),
		cmocka_unit_test(subsample_delay_preserves_pending_interval),
		cmocka_unit_test(first_sample_adds_one_tick),
		cmocka_unit_test(larger_delay_adds_two_ticks),
		cmocka_unit_test(full_buffer_preserves_interval),
		cmocka_unit_test(time_to_frames_rounds_down),
		cmocka_unit_test(mix_whole_tick),
		cmocka_unit_test(mix_from_start_point),
		cmocka_unit_test(mix_outside_tick_adds_nothing),
		cmocka_unit_test(mix_on_frame_1024_adds_nothing),
		cmocka_unit_test(buffering_maxed_only_at_max),
		cmocka_unit_test(fixed_buffering_fills_to_max),
		cmocka_unit_test(fixed_buffering_keeps_buffered_ts_while_waiting),
		cmocka_unit_test(fixed_buffering_at_max_changes_nothing),
		cmocka_unit_test(buffering_stops_at_max),
		cmocka_unit_test(buffering_rounds_ticks_up),
		cmocka_unit_test(core_mix_on_plain_buffers),
		cmocka_unit_test(core_buffering_steps),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
