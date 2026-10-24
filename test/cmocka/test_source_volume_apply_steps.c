#include <float.h>
#include <inttypes.h>
#include <math.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-ptm-gate.h"
#include "obs-source-volume.h"

/*
 * Characterization of the volume decisions of get_source_volume() and
 * apply_audio_volume() in libobs/obs-source.c, moved to
 * libobs/obs-source-volume.c before their Rust port:
 * - obs_source_volume_snap(): the volume a mix applies, 0 when the gate mutes
 *   or within 0.0001 of 0, 1 within 0.0001 of 1, else the volume;
 * - obs_source_volume_action_due(): whether the first queued audio action is
 *   due this tick, action_ts < audio_ts + duration;
 * - obs_source_volume_step(): skip at exactly 1, clear at exactly 0 (or -0)
 *   or with no mixers, else multiply;
 * - obs_source_volume_mixes(): a mix is multiplied when it is in the
 *   source's mixers and the tick's.
 * get_source_volume()'s gate goes through obs-source-ptm-gate.c's helpers
 * (test_source_ptm_gate_steps pins them on their own).
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C sources
 * into itself and calls only them. A stand-in source holds the fields, and
 * get_source_volume() and apply_audio_volume() are copied from obs-source.c
 * onto it, with the memset and multiply_output_audio() recorded instead of
 * run, so the compositions are pinned too. test_source_audio_ranges,
 * test_source_volume_mix, test_audio_capture_mix_race and the scene item
 * audio action tests cover the callers through the real libobs.
 *
 * Since #2128 obs_source_set_volume() and the scene collection loader store
 * only finite volumes of 0 or more, but the functions take any float: the
 * NaN, infinite and negative cases pin what they do with one.
 */

#define MS 1000000ULL
#define SEC (1000 * MS)
/* a clock well away from 0 and UINT64_MAX */
#define T (1000 * SEC)
/* MAX_AUDIO_MIXES, MAX_AUDIO_CHANNELS and AUDIO_OUTPUT_FRAMES of
 * media-io/audio-io.h */
#define MIXES 6
#define CHANNELS 8
#define FRAMES 1024
/* conv_frames_to_time(48000, AUDIO_OUTPUT_FRAMES) */
#define TICK 21333333ULL

static uint32_t fbits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float bitsf(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

static void check_u64(uint64_t actual, uint64_t expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %" PRIu64 ", expected %" PRIu64 "\n", line, what, actual, expected);
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

/* Floats by bit pattern: -0 is not 0, and a NaN must keep its bits. */
static void check_float(float actual, float expected, const char *what, int line)
{
	if (fbits(actual) != fbits(expected)) {
		fprintf(stderr, "line %d: %s is %a (0x%08" PRIx32 "), expected %a (0x%08" PRIx32 ")\n", line, what,
			(double)actual, fbits(actual), (double)expected, fbits(expected));
		fail();
	}
}

#define SNAP(muted, volume, expected) \
	check_float(obs_source_volume_snap((muted), (volume)), (expected), "snap", __LINE__)
/* not snapped: the volume itself, bit for bit */
#define KEPT(volume) SNAP(false, (volume), (volume))
#define DUE(action_ts, audio_ts, duration, expected) \
	check_bool(obs_source_volume_action_due((action_ts), (audio_ts), (duration)), (expected), "due", __LINE__)
#define STEP(vol, mixers, expected) \
	check_u64((uint64_t)obs_source_volume_step((vol), (mixers)), (uint64_t)(expected), "step", __LINE__)

/* The last float below 1 that snaps to 1 and the first that does not:
 * 1 - k * 2^-24 snaps while k * 2^-24 <= 0.0001f, k <= 1677.72. Above 1 the
 * floats are 2^-23 apart: k <= 838.86. */
#define ONE_LOW_IN (1.0f - 1677.0f / 16777216.0f)
#define ONE_LOW_OUT (1.0f - 1678.0f / 16777216.0f)
#define ONE_HIGH_IN (1.0f + 838.0f / 8388608.0f)
#define ONE_HIGH_OUT (1.0f + 839.0f / 8388608.0f)

/* -------------------------------------------------------------------------
 * obs_source_volume_snap() */

/* Within 0.0001f of 0 the volume is 0, +0 even from -0 or a negative
 * volume; one float further it is kept. */
static void snap_zero_edge(void **state)
{
	(void)state;

	SNAP(false, 0.0f, 0.0f);
	/* -0 snaps to +0 */
	SNAP(false, -0.0f, 0.0f);
	SNAP(false, 0.0001f, 0.0f);
	SNAP(false, -0.0001f, 0.0f);
	SNAP(false, nextafterf(0.0001f, 0.0f), 0.0f);
	SNAP(false, nextafterf(-0.0001f, 0.0f), 0.0f);
	KEPT(nextafterf(0.0001f, 1.0f));
	KEPT(nextafterf(-0.0001f, -1.0f));
	KEPT(nextafterf(nextafterf(0.0001f, 1.0f), 1.0f));
	/* the smallest subnormal and FLT_MIN, of either sign */
	SNAP(false, bitsf(1), 0.0f);
	SNAP(false, bitsf(0x80000001u), 0.0f);
	SNAP(false, FLT_MIN, 0.0f);
	SNAP(false, -FLT_MIN, 0.0f);
	/* -60 dB is 0.001: kept */
	KEPT(0.001f);
	KEPT(0.00011f);
}

/* Within 0.0001f of 1 the volume is exactly 1. Characterized, not
 * endorsed: 0.9999f, 1.0001f, 1.0f - 0.0001f and 1.0f + 0.0001f all round to
 * floats just outside, so none of them snaps. */
static void snap_one_edge(void **state)
{
	(void)state;

	/* the edges are float neighbours */
	check_u64(fbits(nextafterf(ONE_LOW_IN, 0.0f)), fbits(ONE_LOW_OUT), "below 1", __LINE__);
	check_u64(fbits(nextafterf(ONE_HIGH_IN, 2.0f)), fbits(ONE_HIGH_OUT), "above 1", __LINE__);

	SNAP(false, 1.0f, 1.0f);
	SNAP(false, nextafterf(1.0f, 0.0f), 1.0f);
	SNAP(false, nextafterf(1.0f, 2.0f), 1.0f);
	SNAP(false, ONE_LOW_IN, 1.0f);
	SNAP(false, nextafterf(ONE_LOW_IN, 1.0f), 1.0f);
	KEPT(ONE_LOW_OUT);
	KEPT(nextafterf(ONE_LOW_OUT, 0.0f));
	SNAP(false, ONE_HIGH_IN, 1.0f);
	SNAP(false, nextafterf(ONE_HIGH_IN, 1.0f), 1.0f);
	KEPT(ONE_HIGH_OUT);
	KEPT(nextafterf(ONE_HIGH_OUT, 2.0f));

	check_u64(fbits(1.0f - 0.0001f), fbits(ONE_LOW_OUT), "1.0f - 0.0001f", __LINE__);
	check_u64(fbits(1.0f + 0.0001f), fbits(ONE_HIGH_OUT), "1.0f + 0.0001f", __LINE__);
	check_u64(fbits(0.9999f), fbits(ONE_LOW_OUT), "0.9999f", __LINE__);
	check_u64(fbits(1.0001f), fbits(ONE_HIGH_OUT), "1.0001f", __LINE__);
	KEPT(1.0f - 0.0001f);
	KEPT(1.0f + 0.0001f);
	KEPT(0.9999f);
	KEPT(1.0001f);
	SNAP(false, 0.99995f, 1.0f);
	SNAP(false, 1.00005f, 1.0f);

	/* -1 is 2 away from 1 */
	KEPT(-1.0f);
	KEPT(-nextafterf(1.0f, 0.0f));
}

/* Anything else is kept, bit for bit. */
static void snap_keeps_other_volumes(void **state)
{
	(void)state;

	KEPT(0.5f);
	KEPT(2.0f);
	/* the most the volume controls set, +26 dB */
	KEPT(20.0f);
	KEPT(FLT_MAX);
	KEPT(-FLT_MAX);
	KEPT(-0.5f);
	/* +-inf are infinitely far from 0 and 1 */
	KEPT(INFINITY);
	KEPT(-INFINITY);
	/* NaN compares false both times and passes through with its bits */
	KEPT(bitsf(0x7fc00000u));
	KEPT(bitsf(0xffc00000u));
	KEPT(bitsf(0x7fc00001u));
	KEPT(bitsf(0xffffffffu));
	if (!isnan(obs_source_volume_snap(false, bitsf(0x7f800001u)))) {
		fprintf(stderr, "a signalling NaN is not NaN after the snap\n");
		fail();
	}
}

/* Muted is 0 (+0) whatever the volume, NaN and -0 included. */
static void snap_muted(void **state)
{
	(void)state;

	const float volumes[] = {0.0f,
				 -0.0f,
				 0.0001f,
				 0.5f,
				 1.0f,
				 20.0f,
				 -1.0f,
				 FLT_MAX,
				 INFINITY,
				 -INFINITY,
				 bitsf(1),
				 ONE_LOW_IN,
				 bitsf(0x7fc00000u),
				 bitsf(0xffc00000u)};
	for (size_t i = 0; i < sizeof(volumes) / sizeof(volumes[0]); i++)
		SNAP(true, volumes[i], 0.0f);
}

/* -------------------------------------------------------------------------
 * get_source_volume() */

/* The fields of struct obs_source get_source_volume() reads and writes. */
struct volume_source {
	bool enabled;
	bool muted;
	bool push_to_mute_enabled;
	bool push_to_mute_pressed;
	bool push_to_talk_enabled;
	bool push_to_talk_pressed;
	uint64_t push_to_mute_delay;
	uint64_t push_to_mute_stop_time;
	uint64_t push_to_talk_delay;
	uint64_t push_to_talk_stop_time;
	float volume;
};

/* get_source_volume() of obs-source.c, on the stand-in. */
static float get_source_volume(struct volume_source *source, uint64_t os_time)
{
	if (source->push_to_mute_enabled && source->push_to_mute_pressed)
		source->push_to_mute_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_mute_delay);

	if (source->push_to_talk_enabled && source->push_to_talk_pressed)
		source->push_to_talk_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_talk_delay);

	bool push_to_mute_active = source->push_to_mute_pressed ||
				   obs_source_ptm_gate_held(os_time, source->push_to_mute_stop_time);
	bool push_to_talk_active = source->push_to_talk_pressed ||
				   obs_source_ptm_gate_held(os_time, source->push_to_talk_stop_time);

	bool muted = !source->enabled || source->muted ||
		     obs_source_ptm_gate_mutes(source->push_to_mute_enabled, push_to_mute_active) ||
		     obs_source_ptm_gate_talk_mutes(source->push_to_talk_enabled, push_to_talk_active);

	return obs_source_volume_snap(muted, source->volume);
}

/* Every combination of the six flags, with each gate's stored stop time
 * before, at and after the clock: muted (0) when disabled, muted,
 * push-to-mute enabled and pressed or held, or push-to-talk enabled and
 * neither pressed nor held; else the volume. A pressed key of an enabled
 * gate stores os_time + its delay, and nothing else is stored. */
static void gate_every_flag(void **state)
{
	(void)state;

	const uint64_t stops[] = {0, T - 1, T, T + 1, UINT64_MAX};
	const size_t nstops = sizeof(stops) / sizeof(stops[0]);
	unsigned runs = 0;

	for (unsigned bits = 0; bits < 64; bits++) {
		for (size_t m = 0; m < nstops; m++) {
			for (size_t t = 0; t < nstops; t++) {
				struct volume_source s = {0};
				s.enabled = bits & 1;
				s.muted = bits & 2;
				s.push_to_mute_enabled = bits & 4;
				s.push_to_mute_pressed = bits & 8;
				s.push_to_talk_enabled = bits & 16;
				s.push_to_talk_pressed = bits & 32;
				s.push_to_mute_delay = 300;
				s.push_to_talk_delay = 500;
				s.push_to_mute_stop_time = stops[m];
				s.push_to_talk_stop_time = stops[t];
				s.volume = 0.5f;

				uint64_t ptm_stop = s.push_to_mute_enabled && s.push_to_mute_pressed ? T + 300 * MS
												     : stops[m];
				uint64_t ptt_stop = s.push_to_talk_enabled && s.push_to_talk_pressed ? T + 500 * MS
												     : stops[t];
				bool ptm_active = s.push_to_mute_pressed || T < ptm_stop;
				bool ptt_active = s.push_to_talk_pressed || T < ptt_stop;
				bool muted = !s.enabled || s.muted || (s.push_to_mute_enabled && ptm_active) ||
					     (s.push_to_talk_enabled && !ptt_active);

				float vol = get_source_volume(&s, T);
				if (fbits(vol) != fbits(muted ? 0.0f : 0.5f)) {
					fprintf(stderr, "flags 0x%02x, stops %zu %zu: volume %a\n", bits, m, t,
						(double)vol);
					fail();
				}
				check_u64(s.push_to_mute_stop_time, ptm_stop, "ptm stop time", __LINE__);
				check_u64(s.push_to_talk_stop_time, ptt_stop, "ptt stop time", __LINE__);
				runs++;
			}
		}
	}
	check_u64(runs, 64 * 25, "runs", __LINE__);
}

/* An open gate gives the snapped volume; a closed one 0 whatever the
 * volume. */
static void gate_then_snap(void **state)
{
	(void)state;

	struct volume_source s = {0};
	s.enabled = true;
	s.push_to_talk_enabled = true;
	s.push_to_talk_delay = 300;

	const float volumes[] = {0.5f, 1.0f, ONE_LOW_IN, ONE_LOW_OUT, 0.0001f, -0.0f, bitsf(0x7fc00000u), INFINITY};
	const float snapped[] = {0.5f, 1.0f, 1.0f, ONE_LOW_OUT, 0.0f, 0.0f, bitsf(0x7fc00000u), INFINITY};
	for (size_t i = 0; i < sizeof(volumes) / sizeof(volumes[0]); i++) {
		s.volume = volumes[i];
		s.push_to_talk_pressed = true;
		check_float(get_source_volume(&s, T), snapped[i], "held", __LINE__);
		s.push_to_talk_pressed = false;
		check_float(get_source_volume(&s, T + 300 * MS - 1), snapped[i], "inside the delay", __LINE__);
		check_float(get_source_volume(&s, T + 300 * MS), 0.0f, "after the delay", __LINE__);
	}
}

/* -------------------------------------------------------------------------
 * obs_source_volume_action_due() */

/* Due strictly before the end of the tick. */
static void action_due_threshold(void **state)
{
	(void)state;

	DUE(T + TICK - 1, T, TICK, true);
	DUE(T + TICK, T, TICK, false);
	DUE(T + TICK + 1, T, TICK, false);
	/* an action from before the tick is due */
	DUE(T - 1, T, TICK, true);
	DUE(0, T, TICK, true);
	DUE(T, T, TICK, true);
	/* a duration of 0: only actions before audio_ts */
	DUE(T - 1, T, 0, true);
	DUE(T, T, 0, false);
	DUE(0, 0, 0, false);
	DUE(0, 0, 1, true);
	DUE(UINT64_MAX, T, TICK, false);
	DUE(UINT64_MAX - 1, UINT64_MAX - TICK, TICK, true);
	DUE(UINT64_MAX - 1, UINT64_MAX, 0, true);
	DUE(UINT64_MAX, UINT64_MAX, 0, false);
}

/* Characterized, not endorsed: audio_ts + duration wraps, so in the last
 * tick before the u64 clock ends nothing at or after audio_ts is due, and
 * only actions before the wrapped sum are. */
static void action_due_wraps(void **state)
{
	(void)state;

	const uint64_t audio_ts = UINT64_MAX - 10;
	/* audio_ts + TICK wraps to TICK - 11 */
	DUE(audio_ts, audio_ts, TICK, false);
	DUE(audio_ts + 5, audio_ts, TICK, false);
	DUE(UINT64_MAX, audio_ts, TICK, false);
	DUE(TICK - 12, audio_ts, TICK, true);
	DUE(TICK - 11, audio_ts, TICK, false);
	DUE(0, audio_ts, TICK, true);
	/* the sum exactly UINT64_MAX does not wrap; one more wraps to 0 */
	DUE(UINT64_MAX - 1, UINT64_MAX - TICK, TICK, true);
	DUE(UINT64_MAX - 1, UINT64_MAX - TICK + 1, TICK, false);
	DUE(0, UINT64_MAX - TICK + 1, TICK, false);
	DUE(0, UINT64_MAX, UINT64_MAX, true);
}

/* -------------------------------------------------------------------------
 * obs_source_volume_step() and obs_source_volume_mixes() */

static const uint32_t mask_edges[] = {0, 1, 2, 0x20, 0x3f, 0x40, 0x41, 0x7f, 0x80, 0xff, 0x80000000u, UINT32_MAX};
#define MASK_EDGES (sizeof(mask_edges) / sizeof(mask_edges[0]))

/* Exactly 1 skips, whatever the mixers (characterized, not endorsed: with
 * no mixers too, where the callers have already cleared the buffers);
 * exactly 0 or -0, or no mixers, clears; anything else, NaN included,
 * multiplies. Only exact compares: the float next to 1 or 0 multiplies. */
static void step_each_volume_and_mask(void **state)
{
	(void)state;

	for (size_t i = 0; i < MASK_EDGES; i++) {
		uint32_t mixers = mask_edges[i];
		enum obs_source_volume_op other = mixers ? OBS_SOURCE_VOLUME_MULTIPLY : OBS_SOURCE_VOLUME_CLEAR;

		STEP(1.0f, mixers, OBS_SOURCE_VOLUME_SKIP);
		STEP(0.0f, mixers, OBS_SOURCE_VOLUME_CLEAR);
		STEP(-0.0f, mixers, OBS_SOURCE_VOLUME_CLEAR);
		STEP(nextafterf(1.0f, 0.0f), mixers, other);
		STEP(nextafterf(1.0f, 2.0f), mixers, other);
		STEP(bitsf(1), mixers, other);
		STEP(bitsf(0x80000001u), mixers, other);
		STEP(0.5f, mixers, other);
		STEP(20.0f, mixers, other);
		STEP(-1.0f, mixers, other);
		STEP(INFINITY, mixers, other);
		STEP(-INFINITY, mixers, other);
		STEP(bitsf(0x7fc00000u), mixers, other);
		STEP(bitsf(0xffc00000u), mixers, other);
		STEP(bitsf(0x7f800001u), mixers, other);
	}
}

static void step_values(void **state)
{
	(void)state;

	check_u64(OBS_SOURCE_VOLUME_SKIP, 0, "SKIP", __LINE__);
	check_u64(OBS_SOURCE_VOLUME_CLEAR, 1, "CLEAR", __LINE__);
	check_u64(OBS_SOURCE_VOLUME_MULTIPLY, 2, "MULTIPLY", __LINE__);
	check_u64(sizeof(enum obs_source_volume_op), sizeof(int), "enum size", __LINE__);
}

/* A mix is multiplied when its bit is in both masks. */
static void mixes_mask_edges(void **state)
{
	(void)state;

	for (size_t a = 0; a < MASK_EDGES; a++) {
		for (size_t b = 0; b < MASK_EDGES; b++) {
			for (size_t mix = 0; mix < MIXES; mix++) {
				uint32_t bit = 1u << mix;
				bool expected = (mask_edges[a] & bit) && (mask_edges[b] & bit);
				bool got = obs_source_volume_mixes(mask_edges[a], mask_edges[b], mix);
				if (got != expected) {
					fprintf(stderr, "mixes(0x%" PRIx32 ", 0x%" PRIx32 ", %zu) is %d\n",
						mask_edges[a], mask_edges[b], mix, got);
					fail();
				}
			}
		}
	}

	check_bool(obs_source_volume_mixes(0x3f, 0x3f, 5), true, "mix 5", __LINE__);
	check_bool(obs_source_volume_mixes(0x1f, 0x3f, 5), false, "mix 5", __LINE__);
	check_bool(obs_source_volume_mixes(0x3f, 0x1f, 5), false, "mix 5", __LINE__);
	check_bool(obs_source_volume_mixes(UINT32_MAX, 0x40, 5), false, "0x40", __LINE__);
	check_bool(obs_source_volume_mixes(0, UINT32_MAX, 0), false, "no source mixers", __LINE__);
}

/* -------------------------------------------------------------------------
 * apply_audio_volume() */

/* What a run of apply_audio_volume() did. */
struct apply_record {
	bool actions;
	bool cleared;
	size_t clear_size;
	size_t multiplied;
	size_t mix[MIXES];
	size_t channels[MIXES];
	uint32_t vol[MIXES];
};

/* The fields of struct obs_source apply_audio_volume() reads, past those of
 * get_source_volume(). */
struct apply_source {
	struct volume_source v;
	uint64_t audio_ts;
	uint32_t audio_mixers;
	size_t actions;
	uint64_t action_ts;
	struct apply_record rec;
};

/* apply_audio_volume() of obs-source.c, on the stand-in: the queue is the
 * first action's timestamp, conv_frames_to_time() is the duration passed
 * in, and apply_audio_actions(), the memset and multiply_output_audio() are
 * recorded. */
static void apply_audio_volume(struct apply_source *source, uint32_t mixers, size_t channels, uint64_t duration)
{
	bool actions_pending = source->actions > 0;
	float vol;

	if (actions_pending) {
		if (obs_source_volume_action_due(source->action_ts, source->audio_ts, duration)) {
			source->rec.actions = true;
			return;
		}
	}

	vol = get_source_volume(&source->v, source->audio_ts);

	switch (obs_source_volume_step(vol, mixers)) {
	case OBS_SOURCE_VOLUME_SKIP:
		return;
	case OBS_SOURCE_VOLUME_CLEAR:
		source->rec.cleared = true;
		source->rec.clear_size = FRAMES * sizeof(float) * CHANNELS * MIXES;
		return;
	case OBS_SOURCE_VOLUME_MULTIPLY:
	default:
		break;
	}

	for (size_t mix = 0; mix < MIXES; mix++) {
		if (obs_source_volume_mixes(source->audio_mixers, mixers, mix)) {
			size_t n = source->rec.multiplied++;
			source->rec.mix[n] = mix;
			source->rec.channels[n] = channels;
			source->rec.vol[n] = fbits(vol);
		}
	}
}

static struct apply_source open_source(float volume, uint32_t audio_mixers)
{
	struct apply_source s;
	memset(&s, 0, sizeof(s));
	s.v.enabled = true;
	s.v.volume = volume;
	s.audio_ts = T;
	s.audio_mixers = audio_mixers;
	return s;
}

/* A due action hands the tick to apply_audio_actions(): no snap, no clear,
 * no multiply. One that is not due leaves the volume step to run. */
static void apply_due_action_first(void **state)
{
	(void)state;

	struct apply_source s = open_source(0.5f, 0x3f);
	s.actions = 1;
	s.action_ts = T + TICK - 1;
	apply_audio_volume(&s, 0x3f, 2, TICK);
	check_bool(s.rec.actions, true, "actions", __LINE__);
	check_u64(s.rec.multiplied, 0, "multiplied", __LINE__);
	check_bool(s.rec.cleared, false, "cleared", __LINE__);

	s = open_source(0.5f, 0x3f);
	s.actions = 1;
	s.action_ts = T + TICK;
	apply_audio_volume(&s, 0x3f, 2, TICK);
	check_bool(s.rec.actions, false, "actions", __LINE__);
	check_u64(s.rec.multiplied, 6, "multiplied", __LINE__);

	/* the action's timestamp only counts when one is queued */
	s = open_source(0.5f, 0x3f);
	s.action_ts = 0;
	apply_audio_volume(&s, 0x3f, 2, TICK);
	check_bool(s.rec.actions, false, "actions", __LINE__);
	check_u64(s.rec.multiplied, 6, "multiplied", __LINE__);

	/* a muted source with a due action: the actions run, nothing else */
	s = open_source(0.5f, 0x3f);
	s.v.muted = true;
	s.actions = 2;
	s.action_ts = 0;
	apply_audio_volume(&s, 0, 2, TICK);
	check_bool(s.rec.actions, true, "actions", __LINE__);
	check_bool(s.rec.cleared, false, "cleared", __LINE__);
}

/* Each step with each mixer mask edge: a volume of 1 leaves the buffers;
 * 0, a muted source or no mixers clears all of them (every mix and
 * channel, whatever the masks); anything else multiplies, in mix order,
 * the mixes in both masks, by the snapped volume. */
static void apply_each_step_and_mask(void **state)
{
	(void)state;

	for (size_t a = 0; a < MASK_EDGES; a++) {
		for (size_t b = 0; b < MASK_EDGES; b++) {
			uint32_t audio_mixers = mask_edges[a];
			uint32_t mixers = mask_edges[b];

			/* 1, and a volume that snaps to it */
			const float ones[] = {1.0f, ONE_LOW_IN, ONE_HIGH_IN};
			for (size_t i = 0; i < 3; i++) {
				struct apply_source s = open_source(ones[i], audio_mixers);
				apply_audio_volume(&s, mixers, 2, TICK);
				check_bool(s.rec.cleared, false, "1: cleared", __LINE__);
				check_u64(s.rec.multiplied, 0, "1: multiplied", __LINE__);
			}

			/* 0, -0, a volume that snaps to 0, and muted */
			const float zeros[] = {0.0f, -0.0f, 0.0001f, 0.5f};
			for (size_t i = 0; i < 4; i++) {
				struct apply_source s = open_source(zeros[i], audio_mixers);
				s.v.muted = i == 3;
				apply_audio_volume(&s, mixers, 2, TICK);
				check_bool(s.rec.cleared, true, "0: cleared", __LINE__);
				check_u64(s.rec.clear_size, FRAMES * sizeof(float) * CHANNELS * MIXES, "0: size",
					  __LINE__);
				check_u64(s.rec.multiplied, 0, "0: multiplied", __LINE__);
			}

			/* anything else */
			const float others[] = {0.5f, ONE_LOW_OUT, 20.0f, bitsf(0x7fc00000u), INFINITY, -1.0f};
			for (size_t i = 0; i < 6; i++) {
				struct apply_source s = open_source(others[i], audio_mixers);
				apply_audio_volume(&s, mixers, 3, TICK);
				check_bool(s.rec.cleared, mixers == 0, "other: cleared", __LINE__);
				size_t n = 0;
				for (size_t mix = 0; mix < MIXES; mix++) {
					uint32_t bit = 1u << mix;
					if (mixers && (audio_mixers & bit) && (mixers & bit)) {
						check_u64(s.rec.mix[n], mix, "mix", __LINE__);
						check_u64(s.rec.channels[n], 3, "channels", __LINE__);
						check_u64(s.rec.vol[n], fbits(others[i]), "vol", __LINE__);
						n++;
					}
				}
				check_u64(s.rec.multiplied, n, "other: multiplied", __LINE__);
			}
		}
	}
}

/* Characterized, not endorsed: mixers with no bit below MAX_AUDIO_MIXES
 * (0x40 and up) are not 0, so nothing is cleared, and no mix is in both
 * masks, so nothing is multiplied either: the buffers keep the unscaled
 * audio. The callers have already cleared every mix that is not in the
 * tick's mixers, so they are silent. */
static void apply_mixers_past_the_mixes(void **state)
{
	(void)state;

	const uint32_t high[] = {0x40, 0x80, 0xc0, 0x80000000u, 0xffffffc0u};
	for (size_t i = 0; i < sizeof(high) / sizeof(high[0]); i++) {
		struct apply_source s = open_source(0.5f, UINT32_MAX);
		apply_audio_volume(&s, high[i], 2, TICK);
		check_bool(s.rec.cleared, false, "cleared", __LINE__);
		check_u64(s.rec.multiplied, 0, "multiplied", __LINE__);
	}

	/* a source in no mix: nothing multiplied, nothing cleared */
	struct apply_source s = open_source(0.5f, 0);
	apply_audio_volume(&s, 0x3f, 2, TICK);
	check_bool(s.rec.cleared, false, "cleared", __LINE__);
	check_u64(s.rec.multiplied, 0, "multiplied", __LINE__);

	/* every mix: all six, in order */
	s = open_source(0.25f, UINT32_MAX);
	apply_audio_volume(&s, UINT32_MAX, 8, TICK);
	check_u64(s.rec.multiplied, 6, "multiplied", __LINE__);
	for (size_t mix = 0; mix < MIXES; mix++) {
		check_u64(s.rec.mix[mix], mix, "mix", __LINE__);
		check_u64(s.rec.channels[mix], 8, "channels", __LINE__);
		check_u64(s.rec.vol[mix], fbits(0.25f), "vol", __LINE__);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(snap_zero_edge),
		cmocka_unit_test(snap_one_edge),
		cmocka_unit_test(snap_keeps_other_volumes),
		cmocka_unit_test(snap_muted),
		cmocka_unit_test(gate_every_flag),
		cmocka_unit_test(gate_then_snap),
		cmocka_unit_test(action_due_threshold),
		cmocka_unit_test(action_due_wraps),
		cmocka_unit_test(step_values),
		cmocka_unit_test(step_each_volume_and_mask),
		cmocka_unit_test(mixes_mask_edges),
		cmocka_unit_test(apply_due_action_first),
		cmocka_unit_test(apply_each_step_and_mask),
		cmocka_unit_test(apply_mixers_past_the_mixes),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
