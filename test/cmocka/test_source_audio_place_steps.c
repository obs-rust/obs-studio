#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-source-audio-place.h"

/*
 * Characterization of the source audio placement and action timeline in
 * libobs/obs-source.c, moved to libobs/obs-source-audio-place.c before its
 * Rust port:
 * - obs_source_audio_place_started() and _older(): the reset of
 *   source_output_audio_place() (no audio_ts, or an older packet), the
 *   push-back rule of source_output_audio_data() and the clamp of
 *   apply_audio_actions();
 * - obs_source_audio_place_size(), _offset(), _fits() and _pop_back(): the
 *   packet's size, its byte placement (get_buf_placement()), the drop past
 *   MAX_BUF_SIZE and how much is popped off the back after the place;
 * - obs_source_audio_place_outputs(): nothing for a monitor-only source;
 * - obs_source_audio_place_action_frame(), _action_stops(), _fill_end() and
 *   _mix_on(): each action's frame, the stop at AUDIO_OUTPUT_FRAMES, how far
 *   the previous volume fills and the mixes multiplied.
 * The last cases run copies of source_output_audio_place(),
 * source_output_audio_push_back(), the last step of
 * source_output_audio_data() and apply_audio_actions() on a stand-in source,
 * so the composition is pinned too, at 8000, 44100, 48000 and 384000 Hz.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_async_delay_audio,
 * test_audio_sync_offset_change, test_audio_clock_drift,
 * test_audio_buffering_subsample, test_media_sync_offset_restart,
 * test_audio_restart_resampler_tail, test_source_volume_mix,
 * test_source_audio_ranges and test_audio_controls cover the callers through
 * the real libobs.
 */

#define FRAMES OBS_SOURCE_AUDIO_PLACE_FRAMES
#define MAX_BUF OBS_SOURCE_AUDIO_PLACE_MAX_BUF_SIZE
#define SEC 1000000000ULL

static const uint32_t RATES[] = {8000, 44100, 48000, 384000};
#define RATE_COUNT (sizeof(RATES) / sizeof(RATES[0]))

/* per rate: the last ns that is still frame 0, and the first that is frame
 * 1; the last ns that is frame 1023, and the first that is frame 1024 */
static const uint64_t FRAME0_LAST[] = {124999, 22675, 20833, 2604};
static const uint64_t FRAME1_FIRST[] = {125000, 22676, 20834, 2605};
static const uint64_t FRAME1023_LAST[] = {127999999, 23219954, 21333333, 2666666};
static const uint64_t FRAME1024_FIRST[] = {128000000, 23219955, 21333334, 2666667};

static const uint64_t TS[] = {
	0, 1, 2, 1000, SEC, (uint64_t)INT64_MAX, (uint64_t)INT64_MAX + 1, UINT64_MAX - 1, UINT64_MAX,
};
#define TS_COUNT (sizeof(TS) / sizeof(TS[0]))

#define CHECK_STR(a, b)                                                                     \
	do {                                                                                \
		if (strcmp((a), (b)) != 0) {                                                \
			fprintf(stderr, "%s:%d: got \"%s\", expected \"%s\"\n", __FILE__, __LINE__, \
				(a), (b));                                                  \
			fail();                                                             \
		}                                                                           \
	} while (0)

static void constants(void **state)
{
	(void)state;

	/* the header's mirrors of the core */
	assert_int_equal(OBS_SOURCE_AUDIO_PLACE_FRAMES, AUDIO_OUTPUT_FRAMES);
	assert_int_equal(OBS_SOURCE_AUDIO_PLACE_FRAMES, 1024);
	assert_int_equal(OBS_SOURCE_AUDIO_PLACE_MAX_BUF_SIZE, 1000 * AUDIO_OUTPUT_FRAMES * sizeof(float));
	assert_int_equal(OBS_SOURCE_AUDIO_PLACE_MAX_BUF_SIZE, 4096000);
	assert_int_equal(OBS_SOURCE_AUDIO_PLACE_MONITOR_ONLY, (int)OBS_MONITORING_TYPE_MONITOR_ONLY);
	assert_int_equal(OBS_SOURCE_AUDIO_PLACE_MONITOR_ONLY, 1);
	assert_int_equal(sizeof(enum obs_monitoring_type), sizeof(int));
}

static void started_and_older(void **state)
{
	(void)state;

	assert_false(obs_source_audio_place_started(0));
	assert_true(obs_source_audio_place_started(1));
	assert_true(obs_source_audio_place_started(UINT64_MAX));

	for (size_t i = 0; i < TS_COUNT; i++) {
		assert_int_equal(obs_source_audio_place_started(TS[i]), TS[i] != 0);
		for (size_t j = 0; j < TS_COUNT; j++)
			assert_int_equal(obs_source_audio_place_older(TS[i], TS[j]), TS[i] < TS[j]);
	}

	/* the same time is not older; one ns before is */
	assert_false(obs_source_audio_place_older(SEC, SEC));
	assert_true(obs_source_audio_place_older(SEC - 1, SEC));
	assert_false(obs_source_audio_place_older(SEC + 1, SEC));
	/* nothing is older than 0 */
	assert_false(obs_source_audio_place_older(0, 0));
}

static void size_bytes(void **state)
{
	(void)state;

	assert_int_equal(obs_source_audio_place_size(0), 0);
	assert_int_equal(obs_source_audio_place_size(1), 4);
	assert_int_equal(obs_source_audio_place_size(480), 1920);
	assert_int_equal(obs_source_audio_place_size(1024), 4096);
	/* in size_t: the largest packet does not wrap */
	assert_int_equal(obs_source_audio_place_size(UINT32_MAX), (size_t)UINT32_MAX * 4);
}

static void offset_per_rate(void **state)
{
	(void)state;

	const uint64_t base = 1000 * SEC;

	for (size_t r = 0; r < RATE_COUNT; r++) {
		uint32_t rate = RATES[r];

		/* placed at 0 */
		assert_int_equal(obs_source_audio_place_offset(base, base, rate), 0);
		/* characterized, not endorsed: less than one sample after
		 * audio_ts rounds down to 0, onto the samples already there */
		assert_int_equal(obs_source_audio_place_offset(base + FRAME0_LAST[r], base, rate), 0);
		/* one frame, in bytes */
		assert_int_equal(obs_source_audio_place_offset(base + FRAME1_FIRST[r], base, rate), 4);
		/* one second */
		assert_int_equal(obs_source_audio_place_offset(base + SEC, base, rate), (size_t)rate * 4);
		/* the timestamp alone does not matter, only the difference */
		assert_int_equal(obs_source_audio_place_offset(SEC, 0, rate), (size_t)rate * 4);
		assert_int_equal(obs_source_audio_place_offset(UINT64_MAX, UINT64_MAX - SEC, rate), (size_t)rate * 4);
	}

	/* 21.312 s at 48 kHz: 1022976 frames, one block short of MAX_BUF_SIZE */
	assert_int_equal(obs_source_audio_place_offset(base + 21312000000ULL, base, 48000), MAX_BUF - 4096);

	/* characterized, not endorsed: a timestamp before audio_ts wraps to a
	 * placement far past MAX_BUF_SIZE (the place resets first, so it never
	 * gets one) */
	assert_int_equal(obs_source_audio_place_offset(base - 1, base, 48000), (size_t)885443715538058ULL * 4);
	/* a rate of 0 places everything at 0 */
	assert_int_equal(obs_source_audio_place_offset(base + SEC, base, 0), 0);
}

static void fits_and_pop_back(void **state)
{
	(void)state;

	assert_true(obs_source_audio_place_fits(0, 0));
	assert_true(obs_source_audio_place_fits(0, 4096));
	assert_true(obs_source_audio_place_fits(0, MAX_BUF));
	assert_false(obs_source_audio_place_fits(0, MAX_BUF + 1));
	/* just under, at and just over MAX_BUF_SIZE */
	assert_true(obs_source_audio_place_fits(MAX_BUF - 4097, 4096));
	assert_true(obs_source_audio_place_fits(MAX_BUF - 4096, 4096));
	assert_false(obs_source_audio_place_fits(MAX_BUF - 4095, 4096));
	assert_true(obs_source_audio_place_fits(MAX_BUF, 0));
	assert_false(obs_source_audio_place_fits(MAX_BUF + 1, 0));
	/* characterized, not endorsed: the sum wraps as size_t, so a start and
	 * size whose sum passes SIZE_MAX fit (neither caller can reach it) */
	assert_true(obs_source_audio_place_fits(SIZE_MAX, 1));
	assert_true(obs_source_audio_place_fits(SIZE_MAX - 9, 10 + MAX_BUF));
	assert_false(obs_source_audio_place_fits(SIZE_MAX, 0));

	/* after deque_place() the channel holds at least placement + size: the
	 * rest is popped off */
	assert_int_equal(obs_source_audio_place_pop_back(8192, 0, 4096), 4096);
	assert_int_equal(obs_source_audio_place_pop_back(8192, 4096, 4096), 0);
	assert_int_equal(obs_source_audio_place_pop_back(MAX_BUF, 4, 4096), MAX_BUF - 4100);
	/* wrapping, as size_t */
	assert_int_equal(obs_source_audio_place_pop_back(0, 0, 1), SIZE_MAX);
	assert_int_equal(obs_source_audio_place_pop_back(5, SIZE_MAX, 1), 5);
}

static void outputs_each_type(void **state)
{
	(void)state;

	assert_true(obs_source_audio_place_outputs(OBS_MONITORING_TYPE_NONE));
	assert_false(obs_source_audio_place_outputs(OBS_MONITORING_TYPE_MONITOR_ONLY));
	assert_true(obs_source_audio_place_outputs(OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT));

	/* characterized, not endorsed: a value outside the enum outputs */
	for (int type = -64; type <= 64; type++)
		assert_int_equal(obs_source_audio_place_outputs(type), type != 1);
	assert_true(obs_source_audio_place_outputs(INT_MIN));
	assert_true(obs_source_audio_place_outputs(INT_MAX));
}

static void action_frames(void **state)
{
	(void)state;

	const uint64_t base = 7 * SEC;

	for (size_t r = 0; r < RATE_COUNT; r++) {
		size_t rate = RATES[r];

		assert_int_equal(obs_source_audio_place_action_frame(base, base, rate), 0);
		assert_int_equal(obs_source_audio_place_action_frame(base + FRAME0_LAST[r], base, rate), 0);
		assert_int_equal(obs_source_audio_place_action_frame(base + FRAME1_FIRST[r], base, rate), 1);
		assert_int_equal(obs_source_audio_place_action_frame(base + FRAME1023_LAST[r], base, rate), FRAMES - 1);
		assert_int_equal(obs_source_audio_place_action_frame(base + FRAME1024_FIRST[r], base, rate), FRAMES);
		assert_int_equal(obs_source_audio_place_action_frame(base + SEC, base, rate), rate);
	}

	/* characterized, not endorsed: unclamped, a timestamp before audio_ts
	 * wraps (apply_audio_actions() clamps first) */
	assert_int_equal(obs_source_audio_place_action_frame(base - 1, base, 48000), 885443715538058ULL);

	/* the stop */
	assert_false(obs_source_audio_place_action_stops(0));
	assert_false(obs_source_audio_place_action_stops(FRAMES - 1));
	assert_true(obs_source_audio_place_action_stops(FRAMES));
	assert_true(obs_source_audio_place_action_stops(FRAMES + 1));
	assert_true(obs_source_audio_place_action_stops(SIZE_MAX));
}

static void fill_ends(void **state)
{
	(void)state;

	/* the previous volume fills up to the action's frame */
	assert_int_equal(obs_source_audio_place_fill_end(0, 0), 0);
	assert_int_equal(obs_source_audio_place_fill_end(0, 1), 1);
	assert_int_equal(obs_source_audio_place_fill_end(0, FRAMES - 1), FRAMES - 1);
	assert_int_equal(obs_source_audio_place_fill_end(100, 200), 200);
	/* an action at or before the filled frames fills nothing */
	assert_int_equal(obs_source_audio_place_fill_end(200, 200), 200);
	assert_int_equal(obs_source_audio_place_fill_end(200, 100), 200);
	assert_int_equal(obs_source_audio_place_fill_end(SIZE_MAX, 0), SIZE_MAX);
	assert_int_equal(obs_source_audio_place_fill_end(0, SIZE_MAX), SIZE_MAX);
}

static void mixes_every_mask(void **state)
{
	(void)state;

	for (uint32_t mask = 0; mask < 256; mask++) {
		for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
			assert_int_equal(obs_source_audio_place_mix_on(mask, mix), (mask >> mix) & 1);
	}

	/* bits past the mixes do not matter for the mixes */
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		assert_true(obs_source_audio_place_mix_on(UINT32_MAX, mix));
		assert_false(obs_source_audio_place_mix_on(~((uint32_t)1 << mix), mix));
		assert_false(obs_source_audio_place_mix_on(0xffffffc0u, mix));
	}
}

/* ---- the callers, copied onto a stand-in source ---- */

struct standin {
	uint64_t audio_ts;
	size_t buf[MAX_AUDIO_CHANNELS];
	size_t channels;
	uint32_t sample_rate;
	int monitoring_type;
	char trace[1024];
};

static void trace(struct standin *s, const char *fmt, ...)
{
	size_t len = strlen(s->trace);
	va_list args;
	va_start(args, fmt);
	vsnprintf(s->trace + len, sizeof(s->trace) - len, fmt, args);
	va_end(args);
}

/* reset_audio_data() */
static void standin_reset(struct standin *s, uint64_t ts)
{
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
		s->buf[i] = 0;
	s->audio_ts = ts;
	trace(s, "reset ");
}

/* source_output_audio_place() */
static void standin_place(struct standin *s, uint64_t timestamp, uint32_t frames)
{
	size_t size = obs_source_audio_place_size(frames);

	if (!obs_source_audio_place_started(s->audio_ts) || obs_source_audio_place_older(timestamp, s->audio_ts))
		standin_reset(s, timestamp);

	size_t placement = obs_source_audio_place_offset(timestamp, s->audio_ts, s->sample_rate);

	if (!obs_source_audio_place_fits(placement, size)) {
		trace(s, "drop(%zu+%zu) ", placement, size);
		return;
	}

	for (size_t i = 0; i < s->channels; i++) {
		/* deque_place() grows the channel to placement + size */
		if (s->buf[i] < placement + size)
			s->buf[i] = placement + size;
		size_t pop = obs_source_audio_place_pop_back(s->buf[i], placement, size);
		s->buf[i] -= pop;
	}
	trace(s, "place(%zu,%zu) ", placement, size);
}

/* source_output_audio_push_back() */
static void standin_push_back(struct standin *s, uint32_t frames)
{
	size_t size = obs_source_audio_place_size(frames);

	if (!obs_source_audio_place_fits(s->buf[0], size)) {
		trace(s, "drop(%zu+%zu) ", s->buf[0], size);
		return;
	}

	for (size_t i = 0; i < s->channels; i++)
		s->buf[i] += size;
	trace(s, "push(%zu) ", size);
}

/* the last step of source_output_audio_data() */
static void standin_output(struct standin *s, bool push_back, uint64_t timestamp, uint32_t frames)
{
	if (obs_source_audio_place_outputs(s->monitoring_type)) {
		if (push_back && obs_source_audio_place_started(s->audio_ts))
			standin_push_back(s, frames);
		else
			standin_place(s, timestamp, frames);
	} else {
		trace(s, "skip ");
	}
}

static struct standin standin_new(uint32_t rate, uint64_t audio_ts)
{
	struct standin s;
	memset(&s, 0, sizeof(s));
	s.audio_ts = audio_ts;
	s.channels = 2;
	s.sample_rate = rate;
	s.monitoring_type = OBS_MONITORING_TYPE_NONE;
	return s;
}

static void place_sequence(void **state)
{
	(void)state;

	const uint64_t base = 1000 * SEC;

	for (size_t r = 0; r < RATE_COUNT; r++) {
		uint32_t rate = RATES[r];
		char expected[256];

		/* no audio_ts: reset, placed at 0 */
		struct standin s = standin_new(rate, 0);
		standin_place(&s, base, 1024);
		CHECK_STR(s.trace, "reset place(0,4096) ");
		assert_int_equal(s.audio_ts, base);
		assert_int_equal(s.buf[0], 4096);
		assert_int_equal(s.buf[1], 4096);
		assert_int_equal(s.buf[2], 0);

		/* one frame later: overlaps all but the first frame, and the
		 * channel is cut to its end */
		s.trace[0] = 0;
		standin_place(&s, base + FRAME1_FIRST[r], 1024);
		CHECK_STR(s.trace, "place(4,4096) ");
		assert_int_equal(s.buf[0], 4100);

		/* less than a sample after audio_ts: on top of the start, and
		 * the longer buffer is cut back */
		s.trace[0] = 0;
		standin_place(&s, base + FRAME0_LAST[r], 480);
		CHECK_STR(s.trace, "place(0,1920) ");
		assert_int_equal(s.buf[0], 1920);

		/* an older packet: reset, placed at 0 at its own time */
		s.trace[0] = 0;
		standin_place(&s, base - 1, 1024);
		CHECK_STR(s.trace, "reset place(0,4096) ");
		assert_int_equal(s.audio_ts, base - 1);

		/* one second later */
		s = standin_new(rate, base);
		standin_place(&s, base + SEC, 1);
		snprintf(expected, sizeof(expected), "place(%zu,4) ", (size_t)rate * 4);
		CHECK_STR(s.trace, expected);
		assert_int_equal(s.buf[0], (size_t)rate * 4 + 4);
	}

	/* just under, at and just over MAX_BUF_SIZE at 48 kHz: the block that
	 * ends exactly at MAX_BUF_SIZE is placed, one frame later it is
	 * dropped */
	struct standin s = standin_new(48000, base);
	standin_place(&s, base + 21312000000ULL - 20833, 1024);
	CHECK_STR(s.trace, "place(4091900,4096) ");
	s.trace[0] = 0;
	standin_place(&s, base + 21312000000ULL, 1024);
	CHECK_STR(s.trace, "place(4091904,4096) ");
	assert_int_equal(s.buf[0], MAX_BUF);
	s.trace[0] = 0;
	standin_place(&s, base + 21312000000ULL + 20834, 1024);
	CHECK_STR(s.trace, "drop(4091908+4096) ");
	assert_int_equal(s.buf[0], MAX_BUF);

	/* characterized, not endorsed: a packet larger than MAX_BUF_SIZE on
	 * its own is dropped whole, even into an empty buffer */
	s = standin_new(48000, 0);
	standin_place(&s, base, 1024001);
	CHECK_STR(s.trace, "reset drop(0+4096004) ");
	assert_int_equal(s.audio_ts, base);
	s = standin_new(48000, 0);
	standin_place(&s, base, 1024000);
	CHECK_STR(s.trace, "reset place(0,4096000) ");

	/* no channels: placed with nothing to place into */
	s = standin_new(48000, base);
	s.channels = 0;
	standin_place(&s, base, 1024);
	CHECK_STR(s.trace, "place(0,4096) ");
	assert_int_equal(s.buf[0], 0);
}

static void push_back_sequence(void **state)
{
	(void)state;

	struct standin s = standin_new(48000, SEC);
	standin_push_back(&s, 1024);
	standin_push_back(&s, 480);
	CHECK_STR(s.trace, "push(4096) push(1920) ");
	assert_int_equal(s.buf[0], 6016);
	assert_int_equal(s.buf[1], 6016);

	/* just under, at and just over MAX_BUF_SIZE */
	s = standin_new(48000, SEC);
	s.buf[0] = MAX_BUF - 4097;
	standin_push_back(&s, 1024);
	CHECK_STR(s.trace, "push(4096) ");
	s.trace[0] = 0;
	s.buf[0] = MAX_BUF - 4096;
	standin_push_back(&s, 1024);
	CHECK_STR(s.trace, "push(4096) ");
	assert_int_equal(s.buf[0], MAX_BUF);
	s.trace[0] = 0;
	standin_push_back(&s, 1);
	CHECK_STR(s.trace, "drop(4096000+4) ");
	assert_int_equal(s.buf[0], MAX_BUF);

	/* characterized, not endorsed: only the first channel is checked, so a
	 * longer second channel still grows */
	s = standin_new(48000, SEC);
	s.buf[1] = MAX_BUF;
	standin_push_back(&s, 1024);
	CHECK_STR(s.trace, "push(4096) ");
	assert_int_equal(s.buf[1], MAX_BUF + 4096);
}

static void output_each_monitoring_type(void **state)
{
	(void)state;

	const uint64_t base = 50 * SEC;
	const int TYPES[] = {OBS_MONITORING_TYPE_NONE, OBS_MONITORING_TYPE_MONITOR_ONLY,
			     OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT, 3, -1};
	const bool OUTPUTS[] = {true, false, true, true, true};

	for (size_t t = 0; t < sizeof(TYPES) / sizeof(TYPES[0]); t++) {
		for (int push_back = 0; push_back < 2; push_back++) {
			for (int has_ts = 0; has_ts < 2; has_ts++) {
				struct standin s = standin_new(48000, has_ts ? base : 0);
				s.monitoring_type = TYPES[t];
				standin_output(&s, push_back, base + SEC, 1024);

				const char *expected = !OUTPUTS[t]           ? "skip "
						       : push_back && has_ts ? "push(4096) "
						       : has_ts              ? "place(192000,4096) "
									     : "reset place(0,4096) ";
				CHECK_STR(s.trace, expected);
			}
		}
	}
}

/* apply_audio_actions(): each action's frame, the fill and the mixes; the
 * volume after an action is its index + 2 (as get_source_volume() would
 * return after apply_audio_action()), the volume before the first 1 */
struct standin_action {
	uint64_t timestamp;
	bool applied;
};

static size_t standin_actions(uint64_t audio_ts, size_t rate, struct standin_action *actions, size_t count,
			      uint32_t mixers, float *vol_data, bool *mixes)
{
	float cur_vol = 1.0f;
	size_t frame_num = 0;
	size_t applied = 0;

	for (size_t i = 0; i < count; i++) {
		uint64_t timestamp = actions[i].timestamp;

		if (obs_source_audio_place_older(timestamp, audio_ts))
			timestamp = audio_ts;

		size_t new_frame_num = obs_source_audio_place_action_frame(timestamp, audio_ts, rate);

		if (obs_source_audio_place_action_stops(new_frame_num))
			break;

		actions[i].applied = true;
		applied++;

		size_t fill_end = obs_source_audio_place_fill_end(frame_num, new_frame_num);
		for (; frame_num < fill_end; frame_num++)
			vol_data[frame_num] = cur_vol;

		cur_vol = (float)(i + 2);
	}

	for (; frame_num < FRAMES; frame_num++)
		vol_data[frame_num] = cur_vol;

	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		mixes[mix] = obs_source_audio_place_mix_on(mixers, mix);

	return applied;
}

/* the frames of vol_data at v, as "from-to" runs */
static void runs_of(const float *vol_data, char *out, size_t out_size)
{
	size_t start = 0;
	out[0] = 0;
	for (size_t i = 1; i <= FRAMES; i++) {
		if (i == FRAMES || vol_data[i] != vol_data[start]) {
			size_t len = strlen(out);
			snprintf(out + len, out_size - len, "%zu-%zu:%g ", start, i - 1, (double)vol_data[start]);
			start = i;
		}
	}
}

static void action_timeline(void **state)
{
	(void)state;

	const uint64_t base = 20 * SEC;
	float vol_data[FRAMES];
	bool mixes[MAX_AUDIO_MIXES];
	char runs[512];

	for (size_t r = 0; r < RATE_COUNT; r++) {
		size_t rate = RATES[r];

		/* characterized, not endorsed: an action before audio_ts applies
		 * at frame 0, like one at it; one after it at its frame; one at
		 * frame 1023 is the last that applies this tick */
		struct standin_action a[] = {{base - SEC}, {base}, {base + FRAME1_FIRST[r]}, {base + FRAME1023_LAST[r]}};
		size_t applied = standin_actions(base, rate, a, 4, 0x3f, vol_data, mixes);
		assert_int_equal(applied, 4);
		runs_of(vol_data, runs, sizeof(runs));
		CHECK_STR(runs, "0-0:3 1-1022:4 1023-1023:5 ");

		/* an action at frame 1024 stays queued, and so does every one
		 * after it, even one that is due */
		struct standin_action b[] = {{base + FRAME0_LAST[r]}, {base + FRAME1024_FIRST[r]}, {base}};
		applied = standin_actions(base, rate, b, 3, 0x3f, vol_data, mixes);
		assert_int_equal(applied, 1);
		assert_true(b[0].applied);
		assert_false(b[1].applied);
		assert_false(b[2].applied);
		runs_of(vol_data, runs, sizeof(runs));
		CHECK_STR(runs, "0-1023:2 ");

		/* the previous volume fills up to the action's frame (100 at
		 * every rate) */
		struct standin_action c[] = {{base + FRAME1_FIRST[r] * 100}};
		standin_actions(base, rate, c, 1, 0x3f, vol_data, mixes);
		runs_of(vol_data, runs, sizeof(runs));
		CHECK_STR(runs, "0-99:1 100-1023:2 ");

		/* no actions: the volume before them fills the tick */
		standin_actions(base, rate, NULL, 0, 0x3f, vol_data, mixes);
		runs_of(vol_data, runs, sizeof(runs));
		CHECK_STR(runs, "0-1023:1 ");
	}

	/* characterized, not endorsed: actions out of order; a later action at
	 * an earlier frame fills nothing, and its volume starts where the
	 * filled frames end */
	struct standin_action d[] = {{base + 21333333ULL / 2}, {base + 20834}};
	standin_actions(base, 48000, d, 2, 0x3f, vol_data, mixes);
	runs_of(vol_data, runs, sizeof(runs));
	CHECK_STR(runs, "0-510:1 511-1023:3 ");

	/* the mixes multiplied are the mixer bits */
	standin_actions(base, 48000, NULL, 0, 0x15, vol_data, mixes);
	assert_true(mixes[0]);
	assert_false(mixes[1]);
	assert_true(mixes[2]);
	assert_false(mixes[3]);
	assert_true(mixes[4]);
	assert_false(mixes[5]);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),          cmocka_unit_test(started_and_older),
		cmocka_unit_test(size_bytes),         cmocka_unit_test(offset_per_rate),
		cmocka_unit_test(fits_and_pop_back),  cmocka_unit_test(outputs_each_type),
		cmocka_unit_test(action_frames),      cmocka_unit_test(fill_ends),
		cmocka_unit_test(mixes_every_mask),   cmocka_unit_test(place_sequence),
		cmocka_unit_test(push_back_sequence), cmocka_unit_test(output_each_monitoring_type),
		cmocka_unit_test(action_timeline),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
