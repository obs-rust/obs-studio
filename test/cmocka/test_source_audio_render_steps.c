#include <inttypes.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-audio-render.h"

/*
 * Characterization of the audio render mix plan of obs_source_audio_render(),
 * custom_audio_render() and process_audio_source_tick() in
 * libobs/obs-source.c, moved to libobs/obs-source-audio-render.c before its
 * Rust port:
 * - obs_source_audio_render_route(): pending without an output buffer or
 *   with a custom render but no data, the custom render, else the tick;
 * - custom_audio_render(): the mixes cleared before the callback (in both
 *   masks), the timestamp and pending result, nothing more with no timestamp
 *   or no tick mixers, the mixes cleared after it (in the tick's mixers but
 *   not the source's);
 * - process_audio_source_tick(): pending with no timestamp or less input
 *   than the tick needs; mixes 1 to 5 copy mix 0 when in both masks, else
 *   are cleared; a submix source does mix 1 only, from bit 0 of its mixers,
 *   and stops there; any other clears mix 0 unless it is in both masks and
 *   scales by its volume.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. A stand-in source holds the fields, and
 * the three callers are copied from obs-source.c onto it, with the lock,
 * deque_peek_front(), every memset and memcpy, the callbacks and
 * apply_audio_volume() recorded instead of run, so the compositions are
 * pinned too. test_source_volume_mix, test_source_audio_ranges,
 * test_audio_capture_mix_race and test_group_hide_transition_audio cover the
 * callers through the real libobs.
 */

/* MAX_AUDIO_MIXES, MAX_AUDIO_CHANNELS and AUDIO_OUTPUT_FRAMES of
 * media-io/audio-io.h */
#define MIXES 6
#define CHANNELS 8
#define FRAMES 1024
/* the bytes of one channel of a tick, AUDIO_OUTPUT_FRAMES floats */
#define TICK_BYTES (FRAMES * sizeof(float))

static void check_u64(uint64_t actual, uint64_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %" PRIu64 " (0x%" PRIx64 "), expected %" PRIu64 " (0x%" PRIx64 ")\n",
			line, what, actual, actual, expected, expected);
		fail();
	}
}

static void check_bool(bool actual, bool expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %s, expected %s\n", line, what, actual ? "true" : "false",
			expected ? "true" : "false");
		fail();
	}
}

#define CHECK_U64(actual, expected) check_u64((uint64_t)(actual), (uint64_t)(expected), #actual, __LINE__)
#define CHECK_BOOL(actual, expected) check_bool((actual), (expected), #actual, __LINE__)

/* Mixer masks: none, each edge of the six mixes, past them, the top bit
 * and all bits. */
static const uint32_t MASKS[] = {0,    1,    2,     0x20,  0x3e,    0x3f,        0x40,        0x41,        0x7f,
				 0x80, 0xff, 0x100, 0xfc0, 0x10000, 0x7fffffffu, 0x80000000u, 0xfffffffeu, UINT32_MAX};
#define NUM_MASKS (sizeof(MASKS) / sizeof(MASKS[0]))

static bool has_mix(uint32_t mask, size_t mix)
{
	return (mask >> mix) & 1u;
}

/* -------------------------------------------------------------------------
 * obs_source_audio_render_route() */

static void route_values(void **state)
{
	(void)state;

	CHECK_U64(OBS_SOURCE_AUDIO_RENDER_PENDING, 0);
	CHECK_U64(OBS_SOURCE_AUDIO_RENDER_CUSTOM, 1);
	CHECK_U64(OBS_SOURCE_AUDIO_RENDER_TICK, 2);
	CHECK_U64(sizeof(enum obs_source_audio_render_route), sizeof(int));
}

/* Every combination of the three inputs, including the ones the caller
 * never passes (it reads custom only with a buffer and data only with
 * custom): no buffer is pending whatever the rest, a custom render needs
 * data, anything else ticks. */
static void route_every_input(void **state)
{
	(void)state;

	for (int bits = 0; bits < 8; bits++) {
		bool has_buffer = bits & 1, custom = bits & 2, has_data = bits & 4;
		enum obs_source_audio_render_route expected = !has_buffer ? OBS_SOURCE_AUDIO_RENDER_PENDING
							      : !custom   ? OBS_SOURCE_AUDIO_RENDER_TICK
							      : has_data  ? OBS_SOURCE_AUDIO_RENDER_CUSTOM
									  : OBS_SOURCE_AUDIO_RENDER_PENDING;
		CHECK_U64(obs_source_audio_render_route(has_buffer, custom, has_data), expected);
	}

	CHECK_U64(obs_source_audio_render_route(false, false, false), OBS_SOURCE_AUDIO_RENDER_PENDING);
	CHECK_U64(obs_source_audio_render_route(false, true, true), OBS_SOURCE_AUDIO_RENDER_PENDING);
	CHECK_U64(obs_source_audio_render_route(true, true, false), OBS_SOURCE_AUDIO_RENDER_PENDING);
	CHECK_U64(obs_source_audio_render_route(true, true, true), OBS_SOURCE_AUDIO_RENDER_CUSTOM);
	/* data does not matter without a custom render */
	CHECK_U64(obs_source_audio_render_route(true, false, false), OBS_SOURCE_AUDIO_RENDER_TICK);
	CHECK_U64(obs_source_audio_render_route(true, false, true), OBS_SOURCE_AUDIO_RENDER_TICK);
}

/* -------------------------------------------------------------------------
 * custom_audio_render()'s decisions */

/* Cleared before the callback: the mixes in both masks. */
static void custom_clear_before_masks(void **state)
{
	(void)state;

	CHECK_BOOL(obs_source_audio_render_custom_clear_before(0x3f, 0x3f, 0), true);
	CHECK_BOOL(obs_source_audio_render_custom_clear_before(0x3f, 0x3f, 5), true);
	CHECK_BOOL(obs_source_audio_render_custom_clear_before(0x01, 0x3f, 1), false);
	CHECK_BOOL(obs_source_audio_render_custom_clear_before(0x3f, 0x01, 1), false);
	CHECK_BOOL(obs_source_audio_render_custom_clear_before(0, UINT32_MAX, 0), false);
	CHECK_BOOL(obs_source_audio_render_custom_clear_before(UINT32_MAX, 0, 0), false);
	CHECK_BOOL(obs_source_audio_render_custom_clear_before(UINT32_MAX, UINT32_MAX, 3), true);
	/* bits past the six mixes are never asked for */
	CHECK_BOOL(obs_source_audio_render_custom_clear_before(0x40, 0x40, 0), false);

	for (size_t a = 0; a < NUM_MASKS; a++)
		for (size_t m = 0; m < NUM_MASKS; m++)
			for (size_t mix = 0; mix < MIXES; mix++)
				CHECK_BOOL(obs_source_audio_render_custom_clear_before(MASKS[a], MASKS[m], mix),
					   has_mix(MASKS[a], mix) && has_mix(MASKS[m], mix));
	/* every pair of the six mix bits */
	for (uint32_t a = 0; a < 64; a++)
		for (uint32_t m = 0; m < 64; m++)
			for (size_t mix = 0; mix < MIXES; mix++)
				CHECK_BOOL(obs_source_audio_render_custom_clear_before(a, m, mix), has_mix(a & m, mix));
}

/* The callback's timestamp when it succeeds, else 0 and pending; the
 * timestamp is not looked at when it failed. */
static void custom_result(void **state)
{
	(void)state;

	static const uint64_t TS[] = {0, 1, 1000000000ull, 0x8000000000000000ull, UINT64_MAX};
	for (size_t i = 0; i < sizeof(TS) / sizeof(TS[0]); i++) {
		CHECK_U64(obs_source_audio_render_custom_ts(true, TS[i]), TS[i]);
		CHECK_U64(obs_source_audio_render_custom_ts(false, TS[i]), 0);
	}
	CHECK_BOOL(obs_source_audio_render_custom_pending(true), false);
	CHECK_BOOL(obs_source_audio_render_custom_pending(false), true);
}

/* After a successful callback: nothing is cleared or scaled with no
 * timestamp (the callback said 0) or no tick mixers. Characterized, not
 * endorsed: tick mixers past the six mixes (0x40 and up) are not "no
 * mixers", so the clears look at each mix (none is in the tick) and the
 * volume runs. */
static void custom_unmixed(void **state)
{
	(void)state;

	static const uint64_t TS[] = {0, 1, UINT64_MAX};
	for (size_t t = 0; t < 3; t++)
		for (size_t m = 0; m < NUM_MASKS; m++)
			CHECK_BOOL(obs_source_audio_render_custom_unmixed(TS[t], MASKS[m]),
				   TS[t] == 0 || MASKS[m] == 0);

	CHECK_BOOL(obs_source_audio_render_custom_unmixed(0, UINT32_MAX), true);
	CHECK_BOOL(obs_source_audio_render_custom_unmixed(1, 0), true);
	CHECK_BOOL(obs_source_audio_render_custom_unmixed(1, 1), false);
	CHECK_BOOL(obs_source_audio_render_custom_unmixed(1, 0x40), false);
	CHECK_BOOL(obs_source_audio_render_custom_unmixed(UINT64_MAX, 0x80000000u), false);
}

/* Cleared after the callback: a mix of the tick that the source is not
 * on. */
static void custom_clear_after_masks(void **state)
{
	(void)state;

	for (size_t m = 0; m < NUM_MASKS; m++)
		for (size_t mix = 0; mix < MIXES; mix++) {
			CHECK_BOOL(obs_source_audio_render_custom_in_tick(MASKS[m], mix), has_mix(MASKS[m], mix));
			CHECK_BOOL(obs_source_audio_render_custom_clear_after(MASKS[m], mix), !has_mix(MASKS[m], mix));
		}

	CHECK_BOOL(obs_source_audio_render_custom_in_tick(0, 0), false);
	CHECK_BOOL(obs_source_audio_render_custom_in_tick(1, 0), true);
	CHECK_BOOL(obs_source_audio_render_custom_in_tick(0x20, 5), true);
	CHECK_BOOL(obs_source_audio_render_custom_in_tick(0x40, 5), false);
	CHECK_BOOL(obs_source_audio_render_custom_clear_after(0, 0), true);
	CHECK_BOOL(obs_source_audio_render_custom_clear_after(0x3f, 5), false);
	CHECK_BOOL(obs_source_audio_render_custom_clear_after(0x1f, 5), true);
	CHECK_BOOL(obs_source_audio_render_custom_clear_after(UINT32_MAX, 0), false);
}

/* -------------------------------------------------------------------------
 * process_audio_source_tick()'s decisions */

/* Pending with no timestamp, or less buffered input than the tick reads:
 * at the size edges. */
static void tick_pending_edges(void **state)
{
	(void)state;

	CHECK_BOOL(obs_source_audio_render_tick_unset(0), true);
	CHECK_BOOL(obs_source_audio_render_tick_unset(1), false);
	CHECK_BOOL(obs_source_audio_render_tick_unset(UINT64_MAX), false);

	const size_t size = TICK_BYTES;
	CHECK_BOOL(obs_source_audio_render_tick_short(size - 1, size), true);
	CHECK_BOOL(obs_source_audio_render_tick_short(size, size), false);
	CHECK_BOOL(obs_source_audio_render_tick_short(size + 1, size), false);
	CHECK_BOOL(obs_source_audio_render_tick_short(0, size), true);
	CHECK_BOOL(obs_source_audio_render_tick_short(0, 0), false);
	CHECK_BOOL(obs_source_audio_render_tick_short(SIZE_MAX, size), false);
	CHECK_BOOL(obs_source_audio_render_tick_short(SIZE_MAX - 1, SIZE_MAX), true);
	CHECK_BOOL(obs_source_audio_render_tick_short(SIZE_MAX, SIZE_MAX), false);
	CHECK_BOOL(obs_source_audio_render_tick_short(0, 1), true);
}

/* A submix source stops after mix 1; any other walks mixes 1 to 5. */
static void tick_stop_mixes(void **state)
{
	(void)state;

	for (size_t mix = 0; mix <= 64; mix++) {
		CHECK_BOOL(obs_source_audio_render_tick_stop(false, mix), false);
		CHECK_BOOL(obs_source_audio_render_tick_stop(true, mix), mix > 1);
	}
	CHECK_BOOL(obs_source_audio_render_tick_stop(true, SIZE_MAX), true);
	CHECK_BOOL(obs_source_audio_render_tick_volume(false), true);
	CHECK_BOOL(obs_source_audio_render_tick_volume(true), false);
}

/* Copy mix 0 into the mix when it is in both masks, else clear it; a
 * submix source copies into mix 1 when bit 0 of its mixers is set,
 * whatever the tick's mixers (the C sets both masks to 1 there). */
static void tick_copy_masks(void **state)
{
	(void)state;

	for (uint32_t a = 0; a < 64; a++)
		for (uint32_t m = 0; m < 64; m++)
			for (size_t mix = 1; mix < MIXES; mix++) {
				CHECK_BOOL(obs_source_audio_render_tick_copy(false, a, m, mix),
					   has_mix(a, mix) && has_mix(m, mix));
				CHECK_BOOL(obs_source_audio_render_tick_copy(true, a, m, mix), has_mix(a, 0));
			}
	for (size_t a = 0; a < NUM_MASKS; a++)
		for (size_t m = 0; m < NUM_MASKS; m++)
			for (size_t mix = 0; mix < MIXES; mix++) {
				CHECK_BOOL(obs_source_audio_render_tick_copy(false, MASKS[a], MASKS[m], mix),
					   has_mix(MASKS[a], mix) && has_mix(MASKS[m], mix));
				CHECK_BOOL(obs_source_audio_render_tick_copy(true, MASKS[a], MASKS[m], mix),
					   has_mix(MASKS[a], 0));
			}

	/* submix: mix 1 from bit 0 even with no tick mixers, not from bit 1 */
	CHECK_BOOL(obs_source_audio_render_tick_copy(true, 1, 0, 1), true);
	CHECK_BOOL(obs_source_audio_render_tick_copy(true, 2, UINT32_MAX, 1), false);
	CHECK_BOOL(obs_source_audio_render_tick_copy(false, 2, 2, 1), true);
	CHECK_BOOL(obs_source_audio_render_tick_copy(false, 1, 1, 1), false);
	/* a submix source's mix index is not shifted, so any index answers */
	CHECK_BOOL(obs_source_audio_render_tick_copy(true, 1, 0, 64), true);
}

/* Mix 0 is cleared before the volume unless it is in both masks. */
static void tick_clear_main_masks(void **state)
{
	(void)state;

	for (size_t a = 0; a < NUM_MASKS; a++)
		for (size_t m = 0; m < NUM_MASKS; m++)
			CHECK_BOOL(obs_source_audio_render_tick_clear_main(MASKS[a], MASKS[m]),
				   !(has_mix(MASKS[a], 0) && has_mix(MASKS[m], 0)));
	CHECK_BOOL(obs_source_audio_render_tick_clear_main(1, 1), false);
	CHECK_BOOL(obs_source_audio_render_tick_clear_main(0x3e, 0x3f), true);
	CHECK_BOOL(obs_source_audio_render_tick_clear_main(0x3f, 0x3e), true);
	CHECK_BOOL(obs_source_audio_render_tick_clear_main(UINT32_MAX, UINT32_MAX), false);
	CHECK_BOOL(obs_source_audio_render_tick_clear_main(0, 0), true);
}

/* -------------------------------------------------------------------------
 * The callers, copied from obs-source.c onto a stand-in source */

enum call_kind {
	CALL_LOCK = 1,
	CALL_UNLOCK,
	CALL_PEEK,
	CALL_MEMSET,
	CALL_MEMCPY,
	CALL_RENDER,
	CALL_SUBMIX,
	CALL_VOLUME,
};

struct call {
	int kind;
	uint64_t a, b;
};

#define MAX_CALLS 128

struct stand_in {
	/* fields the callers read */
	bool has_buffer;
	bool custom;
	bool has_data;
	bool has_mix;
	bool submix;
	uint64_t audio_ts;
	bool audio_pending;
	uint32_t audio_mixers;
	size_t input_size;
	/* the audio_render callback's answer */
	bool cb_success;
	uint64_t cb_ts;
	/* recorded calls */
	struct call calls[MAX_CALLS];
	size_t num_calls;
};

static void record(struct stand_in *s, int kind, uint64_t a, uint64_t b)
{
	if (s->num_calls == MAX_CALLS) {
		fprintf(stderr, "too many calls\n");
		fail();
	}
	s->calls[s->num_calls++] = (struct call){kind, a, b};
}

static void custom_audio_render(struct stand_in *s, uint32_t mixers, size_t channels)
{
	bool success;
	uint64_t ts;

	for (size_t mix = 0; mix < MIXES; mix++) {
		if (obs_source_audio_render_custom_clear_before(s->audio_mixers, mixers, mix))
			record(s, CALL_MEMSET, mix, TICK_BYTES * channels);
	}

	record(s, CALL_RENDER, mixers, channels);
	success = s->cb_success;
	ts = s->cb_ts;
	record(s, CALL_LOCK, 0, 0);
	s->audio_ts = obs_source_audio_render_custom_ts(success, ts);
	s->audio_pending = obs_source_audio_render_custom_pending(success);

	if (obs_source_audio_render_custom_pending(success) ||
	    obs_source_audio_render_custom_unmixed(s->audio_ts, mixers)) {
		record(s, CALL_UNLOCK, 0, 0);
		return;
	}

	for (size_t mix = 0; mix < MIXES; mix++) {
		if (!obs_source_audio_render_custom_in_tick(mixers, mix))
			continue;
		if (obs_source_audio_render_custom_clear_after(s->audio_mixers, mix))
			record(s, CALL_MEMSET, mix, TICK_BYTES * channels);
	}

	record(s, CALL_VOLUME, mixers, channels);
	record(s, CALL_UNLOCK, 0, 0);
}

static void process_audio_source_tick(struct stand_in *s, uint32_t mixers, size_t channels, size_t size)
{
	bool audio_submix = s->submix;

	record(s, CALL_LOCK, 0, 0);

	if (obs_source_audio_render_tick_unset(s->audio_ts) ||
	    obs_source_audio_render_tick_short(s->input_size, size)) {
		s->audio_pending = true;
		record(s, CALL_UNLOCK, 0, 0);
		return;
	}

	for (size_t ch = 0; ch < channels; ch++)
		record(s, CALL_PEEK, ch, size);

	for (size_t mix = 1; mix < MIXES; mix++) {
		if (obs_source_audio_render_tick_stop(audio_submix, mix))
			break;

		if (!obs_source_audio_render_tick_copy(audio_submix, s->audio_mixers, mixers, mix)) {
			record(s, CALL_MEMSET, mix, size * channels);
			continue;
		}

		for (size_t ch = 0; ch < channels; ch++)
			record(s, CALL_MEMCPY, mix * CHANNELS + ch, size);
	}

	if (!obs_source_audio_render_tick_volume(audio_submix)) {
		s->audio_pending = false;
		record(s, CALL_UNLOCK, 0, 0);
		return;
	}

	if (obs_source_audio_render_tick_clear_main(s->audio_mixers, mixers))
		record(s, CALL_MEMSET, 0, size * channels);

	record(s, CALL_VOLUME, mixers, channels);
	s->audio_pending = false;
	record(s, CALL_UNLOCK, 0, 0);
}

static void audio_render(struct stand_in *s, uint32_t mixers, size_t channels, size_t size)
{
	const bool has_buffer = s->has_buffer;
	const bool custom = has_buffer && s->custom;
	const bool has_data = custom && s->has_data;

	switch (obs_source_audio_render_route(has_buffer, custom, has_data)) {
	case OBS_SOURCE_AUDIO_RENDER_PENDING:
		s->audio_pending = true;
		return;
	case OBS_SOURCE_AUDIO_RENDER_CUSTOM:
		custom_audio_render(s, mixers, channels);
		return;
	case OBS_SOURCE_AUDIO_RENDER_TICK:
	default:
		break;
	}

	if (s->has_mix)
		record(s, CALL_SUBMIX, channels, 0);

	process_audio_source_tick(s, mixers, channels, size);
}

static void check_calls(const struct stand_in *s, const struct call *expected, size_t n, int line)
{
	bool same = s->num_calls == n;
	for (size_t i = 0; same && i < n; i++)
		same = s->calls[i].kind == expected[i].kind && s->calls[i].a == expected[i].a &&
		       s->calls[i].b == expected[i].b;
	if (!same) {
		fprintf(stderr, "line %d: calls differ\n", line);
		for (size_t i = 0; i < s->num_calls || i < n; i++)
			fprintf(stderr, "  %2zu: got %d %" PRIu64 " %" PRIu64 ", expected %d %" PRIu64 " %" PRIu64 "\n",
				i, i < s->num_calls ? s->calls[i].kind : 0, i < s->num_calls ? s->calls[i].a : 0,
				i < s->num_calls ? s->calls[i].b : 0, i < n ? expected[i].kind : 0,
				i < n ? expected[i].a : 0, i < n ? expected[i].b : 0);
		fail();
	}
}

#define CALLS(s, ...)                                                                          \
	do {                                                                                   \
		const struct call expected_[] = {__VA_ARGS__};                                 \
		check_calls((s), expected_, sizeof(expected_) / sizeof(expected_[0]), __LINE__); \
	} while (0)

static struct stand_in source(void)
{
	struct stand_in s;
	memset(&s, 0, sizeof(s));
	s.has_buffer = true;
	s.has_data = true;
	s.audio_ts = 1000;
	s.audio_mixers = 0x3f;
	s.input_size = TICK_BYTES;
	s.cb_success = true;
	s.cb_ts = 2000;
	return s;
}

/* No output buffer: pending, nothing else, whatever the callbacks. */
static void render_no_buffer(void **state)
{
	(void)state;

	for (int bits = 0; bits < 8; bits++) {
		struct stand_in s = source();
		s.has_buffer = false;
		s.custom = bits & 1;
		s.has_data = bits & 2;
		s.has_mix = bits & 4;
		audio_render(&s, 0x3f, 2, TICK_BYTES);
		CHECK_BOOL(s.audio_pending, true);
		CHECK_U64(s.num_calls, 0);
		CHECK_U64(s.audio_ts, 1000);
	}
}

/* A custom render without data (a source still being created): pending,
 * the callback is not called (432017b2c, cf0a5e5c9). */
static void render_custom_no_data(void **state)
{
	(void)state;

	struct stand_in s = source();
	s.custom = true;
	s.has_data = false;
	s.has_mix = true;
	audio_render(&s, 0x3f, 2, TICK_BYTES);
	CHECK_BOOL(s.audio_pending, true);
	CHECK_U64(s.num_calls, 0);
	CHECK_U64(s.audio_ts, 1000);
}

/* A custom render that succeeds: the mixes in both masks cleared, the
 * callback, then under the lock the timestamp, the mixes of the tick the
 * source is not on cleared, and the volume. A submix callback is not
 * called. */
static void render_custom_success(void **state)
{
	(void)state;

	struct stand_in s = source();
	s.custom = true;
	s.has_mix = true;
	s.audio_pending = true;
	s.audio_mixers = 0x05;
	audio_render(&s, 0x03, 2, TICK_BYTES);
	CALLS(&s, {CALL_MEMSET, 0, 2 * TICK_BYTES}, {CALL_RENDER, 0x03, 2}, {CALL_LOCK, 0, 0},
	      {CALL_MEMSET, 1, 2 * TICK_BYTES}, {CALL_VOLUME, 0x03, 2}, {CALL_UNLOCK, 0, 0});
	CHECK_U64(s.audio_ts, 2000);
	CHECK_BOOL(s.audio_pending, false);

	/* every mix in both masks, so nothing cleared after */
	s = source();
	s.custom = true;
	audio_render(&s, 0x3f, 8, TICK_BYTES);
	CALLS(&s, {CALL_MEMSET, 0, 8 * TICK_BYTES}, {CALL_MEMSET, 1, 8 * TICK_BYTES}, {CALL_MEMSET, 2, 8 * TICK_BYTES},
	      {CALL_MEMSET, 3, 8 * TICK_BYTES}, {CALL_MEMSET, 4, 8 * TICK_BYTES}, {CALL_MEMSET, 5, 8 * TICK_BYTES},
	      {CALL_RENDER, 0x3f, 8}, {CALL_LOCK, 0, 0}, {CALL_VOLUME, 0x3f, 8}, {CALL_UNLOCK, 0, 0});

	/* the source on no mix: nothing before, every tick mix after */
	s = source();
	s.custom = true;
	s.audio_mixers = 0;
	audio_render(&s, 0x21, 1, TICK_BYTES);
	CALLS(&s, {CALL_RENDER, 0x21, 1}, {CALL_LOCK, 0, 0}, {CALL_MEMSET, 0, TICK_BYTES}, {CALL_MEMSET, 5, TICK_BYTES},
	      {CALL_VOLUME, 0x21, 1}, {CALL_UNLOCK, 0, 0});

	/* characterized, not endorsed: tick mixers past the six mixes clear
	 * nothing and still reach the volume */
	s = source();
	s.custom = true;
	audio_render(&s, 0x40, 2, TICK_BYTES);
	CALLS(&s, {CALL_RENDER, 0x40, 2}, {CALL_LOCK, 0, 0}, {CALL_VOLUME, 0x40, 2}, {CALL_UNLOCK, 0, 0});
	s = source();
	s.custom = true;
	s.audio_mixers = UINT32_MAX;
	audio_render(&s, UINT32_MAX, 2, TICK_BYTES);
	CHECK_U64(s.num_calls, 6 + 4);
}

/* A custom render that fails: timestamp 0 and pending, unlocked at once;
 * the mixes in both masks were cleared before it. */
static void render_custom_failure(void **state)
{
	(void)state;

	struct stand_in s = source();
	s.custom = true;
	s.cb_success = false;
	s.cb_ts = 5000;
	s.audio_mixers = 0x01;
	audio_render(&s, 0x3f, 2, TICK_BYTES);
	CALLS(&s, {CALL_MEMSET, 0, 2 * TICK_BYTES}, {CALL_RENDER, 0x3f, 2}, {CALL_LOCK, 0, 0}, {CALL_UNLOCK, 0, 0});
	CHECK_U64(s.audio_ts, 0);
	CHECK_BOOL(s.audio_pending, true);
}

/* A custom render that succeeds with timestamp 0, or with no tick mixers:
 * not pending, but nothing after the callback. */
static void render_custom_unmixed(void **state)
{
	(void)state;

	struct stand_in s = source();
	s.custom = true;
	s.cb_ts = 0;
	s.audio_pending = true;
	audio_render(&s, 0x01, 2, TICK_BYTES);
	CALLS(&s, {CALL_MEMSET, 0, 2 * TICK_BYTES}, {CALL_RENDER, 0x01, 2}, {CALL_LOCK, 0, 0}, {CALL_UNLOCK, 0, 0});
	CHECK_U64(s.audio_ts, 0);
	CHECK_BOOL(s.audio_pending, false);

	s = source();
	s.custom = true;
	audio_render(&s, 0, 2, TICK_BYTES);
	CALLS(&s, {CALL_RENDER, 0, 2}, {CALL_LOCK, 0, 0}, {CALL_UNLOCK, 0, 0});
	CHECK_U64(s.audio_ts, 2000);
	CHECK_BOOL(s.audio_pending, false);
}

/* The tick pending: no timestamp, or less input than size; the submix
 * callback still runs first. At, above and one below the size. */
static void render_tick_pending(void **state)
{
	(void)state;

	struct stand_in s = source();
	s.audio_ts = 0;
	s.has_mix = true;
	audio_render(&s, 0x3f, 2, TICK_BYTES);
	CALLS(&s, {CALL_SUBMIX, 2, 0}, {CALL_LOCK, 0, 0}, {CALL_UNLOCK, 0, 0});
	CHECK_BOOL(s.audio_pending, true);

	s = source();
	s.input_size = TICK_BYTES - 1;
	audio_render(&s, 0x3f, 2, TICK_BYTES);
	CALLS(&s, {CALL_LOCK, 0, 0}, {CALL_UNLOCK, 0, 0});
	CHECK_BOOL(s.audio_pending, true);

	s = source();
	s.input_size = 0;
	s.audio_ts = UINT64_MAX;
	audio_render(&s, 0x3f, 2, 1);
	CHECK_BOOL(s.audio_pending, true);

	/* exactly enough */
	s = source();
	s.audio_pending = true;
	audio_render(&s, 0x01, 1, TICK_BYTES);
	CALLS(&s, {CALL_LOCK, 0, 0}, {CALL_PEEK, 0, TICK_BYTES}, {CALL_MEMSET, 1, TICK_BYTES},
	      {CALL_MEMSET, 2, TICK_BYTES}, {CALL_MEMSET, 3, TICK_BYTES}, {CALL_MEMSET, 4, TICK_BYTES},
	      {CALL_MEMSET, 5, TICK_BYTES}, {CALL_VOLUME, 0x01, 1}, {CALL_UNLOCK, 0, 0});
	CHECK_BOOL(s.audio_pending, false);

	/* more than enough, and a size of 0 with no input */
	s = source();
	s.input_size = TICK_BYTES + 1;
	audio_render(&s, 0x01, 1, TICK_BYTES);
	CHECK_BOOL(s.audio_pending, false);
	s = source();
	s.input_size = 0;
	audio_render(&s, 0x01, 1, 0);
	CHECK_BOOL(s.audio_pending, false);
	CHECK_U64(s.calls[1].kind, CALL_PEEK);
	CHECK_U64(s.calls[1].b, 0);
}

/* The tick: peek every channel into mix 0, copy it into mixes 1 to 5 in
 * both masks (each channel), clear the others (all channels at once, size
 * * channels bytes from channel 0), clear mix 0 unless in both masks, then
 * the volume. Every pair of the six mix bits, and the mask edges. */
static void render_tick_every_mask(void **state)
{
	(void)state;

	for (size_t a = 0; a < 64 + NUM_MASKS; a++)
		for (size_t m = 0; m < 64 + NUM_MASKS; m++) {
			uint32_t audio_mixers = a < 64 ? (uint32_t)a : MASKS[a - 64];
			uint32_t mixers = m < 64 ? (uint32_t)m : MASKS[m - 64];
			struct stand_in s = source();
			s.audio_mixers = audio_mixers;
			audio_render(&s, mixers, 2, TICK_BYTES);

			struct call expected[MAX_CALLS];
			size_t n = 0;
			expected[n++] = (struct call){CALL_LOCK, 0, 0};
			expected[n++] = (struct call){CALL_PEEK, 0, TICK_BYTES};
			expected[n++] = (struct call){CALL_PEEK, 1, TICK_BYTES};
			for (size_t mix = 1; mix < MIXES; mix++) {
				if (has_mix(audio_mixers, mix) && has_mix(mixers, mix)) {
					expected[n++] = (struct call){CALL_MEMCPY, mix * CHANNELS, TICK_BYTES};
					expected[n++] = (struct call){CALL_MEMCPY, mix * CHANNELS + 1, TICK_BYTES};
				} else {
					expected[n++] = (struct call){CALL_MEMSET, mix, 2 * TICK_BYTES};
				}
			}
			if (!(has_mix(audio_mixers, 0) && has_mix(mixers, 0)))
				expected[n++] = (struct call){CALL_MEMSET, 0, 2 * TICK_BYTES};
			expected[n++] = (struct call){CALL_VOLUME, mixers, 2};
			expected[n++] = (struct call){CALL_UNLOCK, 0, 0};
			check_calls(&s, expected, n, __LINE__);
			CHECK_BOOL(s.audio_pending, false);
		}
}

/* A submix source (OBS_SOURCE_SUBMIX, audio lines): mix 1 copies mix 0
 * when bit 0 of the source's mixers is set, else is cleared; mixes 2 to 5
 * and mix 0 are left as they are, the volume is not applied, and the tick's
 * mixers do not matter. The submix callback runs first when there is one.
 * Characterized, not endorsed: mix 0 keeps the peeked input even when the
 * source is on no mix. */
static void render_tick_submix(void **state)
{
	(void)state;

	for (size_t a = 0; a < NUM_MASKS; a++)
		for (size_t m = 0; m < NUM_MASKS; m++) {
			struct stand_in s = source();
			s.submix = true;
			s.has_mix = true;
			s.audio_pending = true;
			s.audio_mixers = MASKS[a];
			audio_render(&s, MASKS[m], 2, TICK_BYTES);
			if (has_mix(MASKS[a], 0))
				CALLS(&s, {CALL_SUBMIX, 2, 0}, {CALL_LOCK, 0, 0}, {CALL_PEEK, 0, TICK_BYTES},
				      {CALL_PEEK, 1, TICK_BYTES}, {CALL_MEMCPY, CHANNELS, TICK_BYTES},
				      {CALL_MEMCPY, CHANNELS + 1, TICK_BYTES}, {CALL_UNLOCK, 0, 0});
			else
				CALLS(&s, {CALL_SUBMIX, 2, 0}, {CALL_LOCK, 0, 0}, {CALL_PEEK, 0, TICK_BYTES},
				      {CALL_PEEK, 1, TICK_BYTES}, {CALL_MEMSET, 1, 2 * TICK_BYTES},
				      {CALL_UNLOCK, 0, 0});
			CHECK_BOOL(s.audio_pending, false);
		}

	/* without a submix callback, and with no channels */
	struct stand_in s = source();
	s.submix = true;
	s.audio_mixers = 1;
	audio_render(&s, 0, 0, TICK_BYTES);
	CALLS(&s, {CALL_LOCK, 0, 0}, {CALL_UNLOCK, 0, 0});
}

/* A plain source with an audio_mix callback but not the SUBMIX flag: the
 * callback runs, then the ordinary tick. */
static void render_mix_callback_without_flag(void **state)
{
	(void)state;

	struct stand_in s = source();
	s.has_mix = true;
	audio_render(&s, 0x01, 1, TICK_BYTES);
	CHECK_U64(s.calls[0].kind, CALL_SUBMIX);
	CHECK_U64(s.calls[s.num_calls - 2].kind, CALL_VOLUME);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(route_values),
		cmocka_unit_test(route_every_input),
		cmocka_unit_test(custom_clear_before_masks),
		cmocka_unit_test(custom_result),
		cmocka_unit_test(custom_unmixed),
		cmocka_unit_test(custom_clear_after_masks),
		cmocka_unit_test(tick_pending_edges),
		cmocka_unit_test(tick_stop_mixes),
		cmocka_unit_test(tick_copy_masks),
		cmocka_unit_test(tick_clear_main_masks),
		cmocka_unit_test(render_no_buffer),
		cmocka_unit_test(render_custom_no_data),
		cmocka_unit_test(render_custom_success),
		cmocka_unit_test(render_custom_failure),
		cmocka_unit_test(render_custom_unmixed),
		cmocka_unit_test(render_tick_pending),
		cmocka_unit_test(render_tick_every_mask),
		cmocka_unit_test(render_tick_submix),
		cmocka_unit_test(render_mix_callback_without_flag),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
