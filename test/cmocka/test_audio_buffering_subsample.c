/* Regression for the uninitialized interval in add_audio_buffering (#838),
 * and characterization of the rest of obs-audio-mix.c (time to frames, the
 * mix window and add loop, the fixed and added buffering steps) before its
 * port to obs-core (#1903), and of obs-audio-discard.c (the drop steps
 * ignore_audio, discard_if_stopped and discard_audio) before its port
 * (#2403), and of obs-audio-min-ts.c (the per-source decisions of the
 * minimum audio timestamp step) before its port.
 * Compile the original private implementation directly: this checks the C
 * buffering algorithm, rather than the Rust swap or real-time scheduling.
 * Unused core functions are discarded at link time. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include "libobs/obs-audio-discard.c.h"
#include "libobs/obs-audio-min-ts.c.h"
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

/* Characterization of obs-audio-discard.c before its port to obs-core
 * (#2403), on plain fields: no struct obs_source. */

struct core_input {
	struct deque buf[MAX_AUDIO_CHANNELS];
	uint64_t audio_ts;
	size_t last_size;
	bool pending;
	bool pending_stop;
	volatile bool timing_set;
	struct obs_audio_input in;
};

/* `frames` floats per channel, value ch * 10000 + index, so the front value
 * says how many were popped. */
static void core_input_init(struct core_input *c, size_t channels, size_t frames, uint64_t audio_ts)
{
	memset(c, 0, sizeof(*c));
	for (size_t ch = 0; ch < channels; ch++) {
		for (size_t i = 0; i < frames; i++) {
			float v = (float)(ch * 10000 + i);
			deque_push_back(&c->buf[ch], &v, sizeof(v));
		}
	}
	c->audio_ts = audio_ts;
	c->timing_set = true;
	c->in.buf = c->buf;
	c->in.audio_ts = &c->audio_ts;
	c->in.last_size = &c->last_size;
	c->in.pending = &c->pending;
	c->in.pending_stop = &c->pending_stop;
	c->in.timing_set = &c->timing_set;
}

static void core_input_free(struct core_input *c)
{
	for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
		deque_free(&c->buf[ch]);
}

static size_t frames_left(const struct core_input *c, size_t ch)
{
	return c->buf[ch].size / sizeof(float);
}

/* Index of the front sample of channel ch: the number popped. */
static size_t front_index(struct core_input *c, size_t ch)
{
	float v;
	deque_peek_front(&c->buf[ch], &v, sizeof(v));
	return (size_t)v - ch * 10000;
}

static void ignore_without_timestamp_clears_channel_0(void **state)
{
	(void)state;
	/* Kept, characterized, not endorsed: each channel pops channel 0's
	 * size, read again for each channel, so once channel 0 is empty the
	 * other channels keep their audio. */
	struct core_input c;
	core_input_init(&c, 2, 100, 0);
	c.last_size = 400;
	struct obs_audio_ignore_step step = obs_audio_discard_ignore(&c.in, 2, 48000, start);
	assert_true(step.no_timestamp);
	assert_false(step.rerender);
	assert_false(step.warn);
	assert_int_equal(step.buffered, 100);
	assert_int_equal(frames_left(&c, 0), 0);
	assert_int_equal(frames_left(&c, 1), 100);
	assert_int_equal(c.last_size, 0);
	assert_int_equal(c.audio_ts, 0);
	/* not a restart */
	assert_false(c.pending);
	assert_true(c.timing_set);
	core_input_free(&c);
}

static void ignore_drop_rounds_up(void **state)
{
	(void)state;
	const uint64_t sample = audio_frames_to_ns(48000, 1);
	struct core_input c;
	struct obs_audio_ignore_step step;

	/* Exactly one sample behind: one dropped, in sync, lag 0. */
	core_input_init(&c, 2, 100, start - sample);
	c.last_size = 400;
	step = obs_audio_discard_ignore(&c.in, 2, 48000, start);
	assert_true(step.rerender);
	assert_false(step.warn);
	assert_int_equal(step.buffered, 100);
	assert_int_equal(step.dropped, 1);
	assert_int_equal(step.lag, 0);
	assert_int_equal(c.audio_ts, start);
	assert_int_equal(front_index(&c, 0), 1);
	assert_int_equal(front_index(&c, 1), 1);
	assert_int_equal(c.last_size, 0);
	core_input_free(&c);

	/* A partial sample more: two dropped, audio_ts past start_ts. */
	core_input_init(&c, 2, 100, start - 30000);
	step = obs_audio_discard_ignore(&c.in, 2, 48000, start);
	assert_true(step.rerender);
	assert_int_equal(step.dropped, 2);
	assert_int_equal(c.audio_ts, start - 30000 + audio_frames_to_ns(48000, 2));
	assert_int_equal(front_index(&c, 0), 2);
	core_input_free(&c);
}

static void ignore_lag_wraps_when_the_drop_overshoots(void **state)
{
	(void)state;
	/* 1 ns behind: one sample (20833 ns at 48 kHz) is dropped, audio_ts
	 * lands after start_ts, and the logged lag (start_ts - audio_ts) wraps.
	 * Kept, characterized, not endorsed: C prints it at LOG_DEBUG. */
	struct core_input c;
	core_input_init(&c, 1, 100, start - 1);
	struct obs_audio_ignore_step step = obs_audio_discard_ignore(&c.in, 1, 48000, start);
	assert_true(step.rerender);
	assert_int_equal(step.dropped, 1);
	assert_int_equal(c.audio_ts, start - 1 + audio_frames_to_ns(48000, 1));
	assert_int_equal(step.lag, (uint64_t)0 - (audio_frames_to_ns(48000, 1) - 1));
	core_input_free(&c);
}

static void ignore_adjusts_one_ns_short(void **state)
{
	(void)state;
	/* At 44.1 kHz one sample is 22675 ns (rounded down): 22676 ns behind,
	 * one dropped, audio_ts lands on start_ts - 1 and is moved to start_ts.
	 * The lag is logged before the adjustment. */
	const uint64_t sample = audio_frames_to_ns(44100, 1);
	struct core_input c;
	core_input_init(&c, 2, 100, start - sample - 1);
	struct obs_audio_ignore_step step = obs_audio_discard_ignore(&c.in, 2, 44100, start);
	assert_int_equal(sample, 22675);
	assert_true(step.rerender);
	assert_int_equal(step.dropped, 1);
	assert_int_equal(step.lag, 1);
	assert_int_equal(c.audio_ts, start);
	core_input_free(&c);
}

static void ignore_restarts_when_buffer_is_short(void **state)
{
	(void)state;
	/* 1 ms (48 samples) behind with 10 buffered: all 10 dropped, still
	 * behind, so the source's audio restarts with a warning. */
	struct core_input c;
	core_input_init(&c, 2, 10, start - 1000000);
	struct obs_audio_ignore_step step = obs_audio_discard_ignore(&c.in, 2, 48000, start);
	const uint64_t after = start - 1000000 + audio_frames_to_ns(48000, 10);
	assert_false(step.rerender);
	assert_int_equal(step.dropped, 10);
	assert_int_equal(step.lag, start - after);
	assert_true(step.warn);
	assert_int_equal(step.warn_lag, start - after);
	assert_int_equal(frames_left(&c, 0), 0);
	assert_int_equal(frames_left(&c, 1), 0);
	assert_true(c.pending);
	assert_int_equal(c.audio_ts, 0);
	assert_false(c.timing_set);
	core_input_free(&c);
}

static void ignore_without_samples_warns_unless_pending(void **state)
{
	(void)state;
	struct core_input c;
	struct obs_audio_ignore_step step;

	/* Not pending: restart with a warning. */
	core_input_init(&c, 2, 0, start - 1000);
	step = obs_audio_discard_ignore(&c.in, 2, 48000, start);
	assert_false(step.rerender);
	assert_int_equal(step.buffered, 0);
	assert_int_equal(step.dropped, 0);
	assert_true(step.warn);
	assert_int_equal(step.warn_lag, 1000);
	assert_true(c.pending);
	assert_int_equal(c.audio_ts, 0);
	assert_false(c.timing_set);
	core_input_free(&c);

	/* Already pending: the same restart, no warning. */
	core_input_init(&c, 2, 0, start - 1000);
	c.pending = true;
	step = obs_audio_discard_ignore(&c.in, 2, 48000, start);
	assert_false(step.warn);
	assert_true(c.pending);
	assert_int_equal(c.audio_ts, 0);
	assert_false(c.timing_set);
	core_input_free(&c);
}

static void ignore_pops_channel_0_size_everywhere(void **state)
{
	(void)state;
	/* Kept, characterized, not endorsed: the count is channel 0's, and
	 * every channel pops it. 1 ms (48 samples) behind with 10 in channel 0
	 * and 20 in channel 1: 10 dropped from each. */
	struct core_input c;
	core_input_init(&c, 2, 10, start - 1000000);
	for (size_t i = 0; i < 10; i++) {
		float v = 10000.0f + 10 + i;
		deque_push_back(&c.buf[1], &v, sizeof(v));
	}
	obs_audio_discard_ignore(&c.in, 2, 48000, start);
	assert_int_equal(frames_left(&c, 0), 0);
	assert_int_equal(frames_left(&c, 1), 10);
	assert_int_equal(front_index(&c, 1), 10);
	core_input_free(&c);
}

static void stop_check_needs_two_unchanged_ticks(void **state)
{
	(void)state;
	struct core_input c;

	/* Empty: nothing. */
	core_input_init(&c, 2, 0, start);
	c.last_size = 40;
	assert_int_equal(obs_audio_discard_if_stopped(&c.in, 2), OBS_AUDIO_STOP_NONE);
	assert_int_equal(c.last_size, 40);
	core_input_free(&c);

	/* The size changed: remembered. */
	core_input_init(&c, 2, 10, start);
	assert_int_equal(obs_audio_discard_if_stopped(&c.in, 2), OBS_AUDIO_STOP_NONE);
	assert_int_equal(c.last_size, 10 * sizeof(float));
	assert_false(c.pending_stop);

	/* Unchanged once: pending_stop, data kept. */
	assert_int_equal(obs_audio_discard_if_stopped(&c.in, 2), OBS_AUDIO_STOP_PENDING);
	assert_true(c.pending_stop);
	assert_int_equal(frames_left(&c, 0), 10);

	/* Unchanged twice: cleared, each channel by its own size. */
	for (size_t i = 0; i < 5; i++) {
		float v = 10000.0f + 10 + i;
		deque_push_back(&c.buf[1], &v, sizeof(v));
	}
	assert_int_equal(obs_audio_discard_if_stopped(&c.in, 2), OBS_AUDIO_STOP_CLEARED);
	assert_false(c.pending_stop);
	assert_int_equal(frames_left(&c, 0), 0);
	assert_int_equal(frames_left(&c, 1), 0);
	assert_int_equal(c.audio_ts, 0);
	assert_int_equal(c.last_size, 0);
	core_input_free(&c);
}

static void discard_early_returns(void **state)
{
	(void)state;
	const uint64_t tick = audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	const struct ts_info ts = {start, start + tick};
	struct core_input c;
	struct obs_audio_discard_step step;

	/* A render source: audio_ts reset, nothing popped. */
	core_input_init(&c, 2, 2000, start);
	step = obs_audio_discard_tick(&c.in, true, 2, 48000, &ts);
	assert_int_equal(step.path, OBS_AUDIO_DISCARD_RENDER_SOURCE);
	assert_int_equal(c.audio_ts, 0);
	assert_int_equal(frames_left(&c, 0), 2000);
	core_input_free(&c);

	/* At or after the tick end: nothing. */
	const uint64_t ahead[] = {ts.end, ts.end + 1};
	for (size_t i = 0; i < 2; i++) {
		core_input_init(&c, 2, 2000, ahead[i]);
		step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
		assert_int_equal(step.path, OBS_AUDIO_DISCARD_AHEAD);
		assert_int_equal(c.audio_ts, ahead[i]);
		assert_int_equal(frames_left(&c, 0), 2000);
		core_input_free(&c);
	}

	/* Behind, not pending: no stop check. */
	core_input_init(&c, 2, 10, start - 2);
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.path, OBS_AUDIO_DISCARD_BEHIND);
	assert_int_equal(step.stop, OBS_AUDIO_STOP_NONE);
	assert_int_equal(c.last_size, 0);
	assert_int_equal(c.audio_ts, start - 2);
	core_input_free(&c);

	/* Behind, pending, with a full tick buffered: no stop check either. */
	core_input_init(&c, 2, AUDIO_OUTPUT_FRAMES, start - 2);
	c.pending = true;
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.path, OBS_AUDIO_DISCARD_BEHIND);
	assert_int_equal(c.last_size, 0);
	core_input_free(&c);

	/* Behind, pending, less than a tick: the stop check runs. */
	core_input_init(&c, 2, 10, start - 2);
	c.pending = true;
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.stop, OBS_AUDIO_STOP_NONE);
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.stop, OBS_AUDIO_STOP_PENDING);
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.path, OBS_AUDIO_DISCARD_BEHIND);
	assert_int_equal(step.stop, OBS_AUDIO_STOP_CLEARED);
	assert_int_equal(frames_left(&c, 0), 0);
	assert_int_equal(c.audio_ts, 0);
	core_input_free(&c);

	/* A start point on AUDIO_OUTPUT_FRAMES, in a two-tick window at 8 kHz
	 * (one tick is exactly 128 ms there): nothing. */
	const uint64_t tick8 = audio_frames_to_ns(8000, AUDIO_OUTPUT_FRAMES);
	const struct ts_info wide = {start, start + 2 * tick8};
	core_input_init(&c, 2, 2000, start + tick8);
	step = obs_audio_discard_tick(&c.in, false, 2, 8000, &wide);
	assert_int_equal(tick8, 128000000);
	assert_int_equal(step.path, OBS_AUDIO_DISCARD_START_AT_END);
	assert_int_equal(c.audio_ts, start + tick8);
	assert_int_equal(frames_left(&c, 0), 2000);
	core_input_free(&c);
}

static void discard_pops_the_tick(void **state)
{
	(void)state;
	const uint64_t tick = audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	const struct ts_info ts = {start, start + tick};
	const size_t channels[] = {1, 2, 8};
	struct core_input c;
	struct obs_audio_discard_step step;

	/* On ts->start and on ts->start - 1: the whole tick. */
	for (size_t i = 0; i < 3; i++) {
		for (uint64_t off = 0; off < 2; off++) {
			core_input_init(&c, channels[i], 1500, start - off);
			c.last_size = 40;
			c.pending_stop = true;
			step = obs_audio_discard_tick(&c.in, false, channels[i], 48000, &ts);
			assert_int_equal(step.path, OBS_AUDIO_DISCARD_DONE);
			assert_int_equal(step.stop, OBS_AUDIO_STOP_NONE);
			for (size_t ch = 0; ch < channels[i]; ch++) {
				assert_int_equal(frames_left(&c, ch), 1500 - AUDIO_OUTPUT_FRAMES);
				assert_int_equal(front_index(&c, ch), AUDIO_OUTPUT_FRAMES);
			}
			assert_int_equal(c.audio_ts, ts.end);
			assert_int_equal(c.last_size, 0);
			assert_false(c.pending_stop);
			core_input_free(&c);
		}
	}

	/* From frame 512's time, which maps to frame 511: 513 frames. */
	core_input_init(&c, 2, 1500, start + audio_frames_to_ns(48000, 512));
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.path, OBS_AUDIO_DISCARD_DONE);
	assert_int_equal(front_index(&c, 0), AUDIO_OUTPUT_FRAMES - 511);
	assert_int_equal(front_index(&c, 1), AUDIO_OUTPUT_FRAMES - 511);
	assert_int_equal(c.audio_ts, ts.end);
	core_input_free(&c);
}

static void discard_waits_for_pending_data(void **state)
{
	(void)state;
	const uint64_t tick = audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	const struct ts_info ts = {start, start + tick};
	struct core_input c;
	struct obs_audio_discard_step step;

	/* Less than the tick: nothing popped, audio_ts moves to the end, the
	 * size is remembered. */
	core_input_init(&c, 2, 100, start);
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.path, OBS_AUDIO_DISCARD_PENDING);
	assert_int_equal(step.stop, OBS_AUDIO_STOP_NONE);
	assert_int_equal(frames_left(&c, 0), 100);
	assert_int_equal(c.audio_ts, ts.end);
	assert_int_equal(c.last_size, 100 * sizeof(float));
	core_input_free(&c);

	/* The same size again: pending stop, audio_ts still moves to the end. */
	core_input_init(&c, 2, 100, start);
	c.last_size = 100 * sizeof(float);
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.stop, OBS_AUDIO_STOP_PENDING);
	assert_true(c.pending_stop);
	assert_int_equal(c.audio_ts, ts.end);
	core_input_free(&c);

	/* And again: cleared, audio_ts stays 0. */
	core_input_init(&c, 2, 100, start);
	c.last_size = 100 * sizeof(float);
	c.pending_stop = true;
	step = obs_audio_discard_tick(&c.in, false, 2, 48000, &ts);
	assert_int_equal(step.path, OBS_AUDIO_DISCARD_PENDING);
	assert_int_equal(step.stop, OBS_AUDIO_STOP_CLEARED);
	assert_int_equal(frames_left(&c, 0), 0);
	assert_int_equal(c.audio_ts, 0);
	core_input_free(&c);
}

/* The wrappers in obs-audio.c pass the right source fields. */
static void source_wrappers_use_the_source_fields(void **state)
{
	(void)state;
	const uint64_t tick = audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	struct ts_info ts = {start, start + tick};
	struct obs_core_audio audio = {0};
	struct obs_source *source = bzalloc(sizeof(*source));
	for (size_t ch = 0; ch < 2; ch++) {
		for (size_t i = 0; i < 1500; i++) {
			float v = (float)i;
			deque_push_back(&source->audio_input_buf[ch], &v, sizeof(v));
		}
	}

	source->audio_ts = start;
	source->last_audio_input_buf_size = 40;
	source->pending_stop = true;
	discard_audio(&audio, source, 2, 48000, &ts);
	assert_int_equal(source->audio_input_buf[1].size, (1500 - AUDIO_OUTPUT_FRAMES) * sizeof(float));
	assert_int_equal(source->audio_ts, ts.end);
	assert_int_equal(source->last_audio_input_buf_size, 0);
	assert_false(source->pending_stop);

	/* 20 ms (960 samples) behind with 476 buffered: all dropped, restart. */
	source->audio_ts = start - 20000000;
	source->timing_set = true;
	assert_false(ignore_audio(source, 2, 48000, start));
	assert_true(source->audio_pending);
	assert_int_equal(source->audio_ts, 0);
	assert_false(source->timing_set);
	assert_int_equal(source->audio_input_buf[1].size, 0);

	for (size_t ch = 0; ch < 2; ch++)
		deque_free(&source->audio_input_buf[ch]);
	bfree(source);
}

/* Characterization of obs-audio-min-ts.c before its port to obs-core. */

static void min_ts_takes_only_earlier_valid_sources(void **state)
{
	(void)state;
	uint64_t min_ts = start;

	/* Pending, or without a timestamp: not taken. */
	assert_false(obs_audio_min_ts_earlier(true, start - 10, &min_ts));
	assert_false(obs_audio_min_ts_earlier(false, 0, &min_ts));
	assert_int_equal(min_ts, start);

	/* Equal: not taken (strict). */
	assert_false(obs_audio_min_ts_earlier(false, start, &min_ts));

	/* Earlier: taken, and the next one must be earlier still. */
	assert_true(obs_audio_min_ts_earlier(false, start - 10, &min_ts));
	assert_int_equal(min_ts, start - 10);
	assert_false(obs_audio_min_ts_earlier(false, start - 10, &min_ts));
	assert_false(obs_audio_min_ts_earlier(false, start - 5, &min_ts));
	assert_true(obs_audio_min_ts_earlier(false, start - 11, &min_ts));
	assert_int_equal(min_ts, start - 11);
}

static void insufficient_skips_render_pending_and_untimed(void **state)
{
	(void)state;
	bool pending;

	pending = false;
	assert_false(obs_audio_min_ts_insufficient(true, &pending, start, 0, 48000, start));
	assert_false(pending);

	pending = true;
	assert_false(obs_audio_min_ts_insufficient(false, &pending, start, 0, 48000, start));
	assert_true(pending);

	pending = false;
	assert_false(obs_audio_min_ts_insufficient(false, &pending, 0, 0, 48000, start));
	assert_false(pending);
}

static void insufficient_needs_the_rest_of_the_tick(void **state)
{
	(void)state;
	const size_t tick = AUDIO_OUTPUT_FRAMES * sizeof(float);
	bool pending;

	/* On min_ts and on min_ts - 1: a whole tick is enough, one sample
	 * less is short and marks the source pending. */
	for (uint64_t off = 0; off < 2; off++) {
		pending = false;
		assert_false(obs_audio_min_ts_insufficient(false, &pending, start - off, tick, 48000, start));
		assert_false(pending);
		assert_true(obs_audio_min_ts_insufficient(false, &pending, start - off, tick - sizeof(float), 48000,
							  start));
		assert_true(pending);
	}

	/* From frame 512's time (frame 511): 513 frames needed. */
	const uint64_t at = start + audio_frames_to_ns(48000, 512);
	pending = false;
	assert_false(obs_audio_min_ts_insufficient(false, &pending, at, (AUDIO_OUTPUT_FRAMES - 511) * sizeof(float),
						   48000, start));
	assert_true(obs_audio_min_ts_insufficient(false, &pending, at, (AUDIO_OUTPUT_FRAMES - 512) * sizeof(float),
						  48000, start));

	/* A start point at and past AUDIO_OUTPUT_FRAMES (>=, where
	 * discard_audio stops only on ==): never short. 8 kHz, where one tick
	 * is exactly 128 ms. */
	const uint64_t tick8 = audio_frames_to_ns(8000, AUDIO_OUTPUT_FRAMES);
	pending = false;
	assert_false(obs_audio_min_ts_insufficient(false, &pending, start + tick8, 0, 8000, start));
	assert_false(obs_audio_min_ts_insufficient(false, &pending, start + 2 * tick8, 0, 8000, start));
	assert_false(pending);
}

static void buffering_choice(void **state)
{
	(void)state;
	assert_int_equal(obs_audio_min_ts_buffering(true, false, start, start), OBS_AUDIO_BUFFERING_FIXED);
	assert_int_equal(obs_audio_min_ts_buffering(true, false, start - 1, start), OBS_AUDIO_BUFFERING_FIXED);
	assert_int_equal(obs_audio_min_ts_buffering(true, true, start - 1, start), OBS_AUDIO_BUFFERING_NONE);
	assert_int_equal(obs_audio_min_ts_buffering(false, false, start - 1, start), OBS_AUDIO_BUFFERING_ADD);
	assert_int_equal(obs_audio_min_ts_buffering(false, true, start - 1, start), OBS_AUDIO_BUFFERING_ADD);
	assert_int_equal(obs_audio_min_ts_buffering(false, false, start, start), OBS_AUDIO_BUFFERING_NONE);
	assert_int_equal(obs_audio_min_ts_buffering(false, false, start + 1, start), OBS_AUDIO_BUFFERING_NONE);
}

/* A source in the audio source list for calc_min_ts. */
static struct obs_source *list_source(struct obs_core_data *data, const char *name, uint64_t audio_ts, size_t frames)
{
	struct obs_source *source = bzalloc(sizeof(*source));
	pthread_mutex_init(&source->audio_buf_mutex, NULL);
	source->context.name = (char *)name;
	source->audio_ts = audio_ts;
	deque_push_back_zero(&source->audio_input_buf[0], frames * sizeof(float));
	/* added at the head, as obs_source_init does */
	source->next_audio_source = data->first_audio_source;
	data->first_audio_source = source;
	return source;
}

static void list_free(struct obs_core_data *data)
{
	struct obs_source *source = data->first_audio_source;
	while (source) {
		struct obs_source *next = (struct obs_source *)source->next_audio_source;
		deque_free(&source->audio_input_buf[0]);
		pthread_mutex_destroy(&source->audio_buf_mutex);
		bfree(source);
		source = next;
	}
	data->first_audio_source = NULL;
}

static void calc_min_ts_through_the_list(void **state)
{
	(void)state;
	const uint64_t tick = audio_frames_to_ns(48000, AUDIO_OUTPUT_FRAMES);
	struct obs_core_data data = {0};
	uint64_t min_ts;
	const char *name;

	/* Two sources with enough audio: the earlier one. */
	list_source(&data, "a", start - 1000, 2 * AUDIO_OUTPUT_FRAMES);
	list_source(&data, "b", start - 2000, 2 * AUDIO_OUTPUT_FRAMES);
	min_ts = start;
	name = calc_min_ts(&data, 48000, &min_ts);
	assert_int_equal(min_ts, start - 2000);
	assert_string_equal(name, "b");
	list_free(&data);

	/* Ties: the first in the list (the newest) stays. */
	list_source(&data, "old", start - 2000, 2 * AUDIO_OUTPUT_FRAMES);
	list_source(&data, "new", start - 2000, 2 * AUDIO_OUTPUT_FRAMES);
	min_ts = start;
	assert_string_equal(calc_min_ts(&data, 48000, &min_ts), "new");
	list_free(&data);

	/* Nothing earlier than the tick: no name, min_ts unchanged. */
	list_source(&data, "a", start + tick / 2, 2 * AUDIO_OUTPUT_FRAMES);
	min_ts = start;
	assert_null(calc_min_ts(&data, 48000, &min_ts));
	assert_int_equal(min_ts, start);
	list_free(&data);

	/* Kept, characterized, not endorsed: the source with the minimum is
	 * short for a tick at its timestamp. The first pass takes it, the
	 * check marks it pending, and the second pass starts from the first
	 * pass's minimum, so with a strict comparison it finds nothing: the
	 * minimum stays that source's timestamp and the name becomes NULL. */
	struct obs_source *short_src = list_source(&data, "short", start - 2000, 10);
	struct obs_source *ok_src = list_source(&data, "ok", start - 1000, 2 * AUDIO_OUTPUT_FRAMES);
	min_ts = start;
	name = calc_min_ts(&data, 48000, &min_ts);
	assert_true(short_src->audio_pending);
	assert_false(ok_src->audio_pending);
	assert_int_equal(min_ts, start - 2000);
	assert_null(name);
	list_free(&data);
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
		cmocka_unit_test(ignore_without_timestamp_clears_channel_0),
		cmocka_unit_test(ignore_drop_rounds_up),
		cmocka_unit_test(ignore_lag_wraps_when_the_drop_overshoots),
		cmocka_unit_test(ignore_adjusts_one_ns_short),
		cmocka_unit_test(ignore_restarts_when_buffer_is_short),
		cmocka_unit_test(ignore_without_samples_warns_unless_pending),
		cmocka_unit_test(ignore_pops_channel_0_size_everywhere),
		cmocka_unit_test(stop_check_needs_two_unchanged_ticks),
		cmocka_unit_test(discard_early_returns),
		cmocka_unit_test(discard_pops_the_tick),
		cmocka_unit_test(discard_waits_for_pending_data),
		cmocka_unit_test(source_wrappers_use_the_source_fields),
		cmocka_unit_test(min_ts_takes_only_earlier_valid_sources),
		cmocka_unit_test(insufficient_skips_render_pending_and_untimed),
		cmocka_unit_test(insufficient_needs_the_rest_of_the_tick),
		cmocka_unit_test(buffering_choice),
		cmocka_unit_test(calc_min_ts_through_the_list),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
