#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <inttypes.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-ptm-gate.h"

/*
 * Characterization of the push-to-mute and push-to-talk gate of
 * source_muted() in libobs/obs-source.c, moved to
 * libobs/obs-source-ptm-gate.c before its Rust port:
 * - obs_source_ptm_gate_stop_time(): push_to_stop_time() of #2058, the
 *   moment a held key stops holding, os_time + delay_ms * 1000000, saturated
 *   at UINT64_MAX where the product or the sum would wrap (#2051).
 *   get_source_volume() uses it too;
 * - obs_source_ptm_gate_held(): a key let go holds strictly before its stop
 *   time;
 * - obs_source_ptm_gate_mutes() and obs_source_ptm_gate_talk_mutes(): the
 *   push-to-mute gate mutes while enabled and active, the push-to-talk gate
 *   while enabled and not active.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. A stand-in source holds the fields, and
 * source_muted() and the gate of get_source_volume() are copied from
 * obs-source.c onto it, so the composition is pinned too.
 * test_source_audio_ranges covers the callers through the real libobs (the
 * keys pressed through the hotkey router, the stop time read back after a
 * packet), and test_wayland_hotkeys and test_portal_shortcuts the press and
 * the release.
 *
 * The delays come from scene collections (obs_load_source() reads a
 * negative one as 0, #2058) and from the setters, which take any uint64_t;
 * os_time is os_gettime_ns() for source_muted() and an audio timestamp for
 * get_source_volume().
 */

#define MS OBS_SOURCE_PTM_GATE_NS_PER_MS
#define SEC (1000 * MS)
/* the largest delay whose product does not wrap: 18446744073709 ms */
#define MAX_DELAY (UINT64_MAX / MS)
/* what that product leaves below UINT64_MAX: 551615 ns */
#define MAX_DELAY_SLACK (UINT64_MAX - MAX_DELAY * MS)
/* a clock well away from 0 and UINT64_MAX */
#define T (1000 * SEC)

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

#define STOP(os_time, delay, expected) \
	check_u64(obs_source_ptm_gate_stop_time((os_time), (delay)), (expected), "stop time", __LINE__)
#define HELD(os_time, stop, expected) \
	check_bool(obs_source_ptm_gate_held((os_time), (stop)), (expected), "held", __LINE__)

/* The fields of struct obs_source the gate reads and writes. */
struct gate_source {
	bool enabled;
	bool user_muted;
	bool muted;
	bool push_to_mute_enabled;
	bool push_to_mute_pressed;
	bool user_push_to_mute_pressed;
	bool push_to_talk_enabled;
	bool push_to_talk_pressed;
	bool user_push_to_talk_pressed;
	uint64_t push_to_mute_delay;
	uint64_t push_to_mute_stop_time;
	uint64_t push_to_talk_delay;
	uint64_t push_to_talk_stop_time;
};

/* source_muted() of obs-source.c, on the stand-in. */
static bool source_muted(struct gate_source *source, uint64_t os_time)
{
	if (source->push_to_mute_enabled && source->user_push_to_mute_pressed)
		source->push_to_mute_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_mute_delay);

	if (source->push_to_talk_enabled && source->user_push_to_talk_pressed)
		source->push_to_talk_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_talk_delay);

	bool push_to_mute_active = source->user_push_to_mute_pressed ||
				   obs_source_ptm_gate_held(os_time, source->push_to_mute_stop_time);
	bool push_to_talk_active = source->user_push_to_talk_pressed ||
				   obs_source_ptm_gate_held(os_time, source->push_to_talk_stop_time);

	return !source->enabled || source->user_muted ||
	       obs_source_ptm_gate_mutes(source->push_to_mute_enabled, push_to_mute_active) ||
	       obs_source_ptm_gate_talk_mutes(source->push_to_talk_enabled, push_to_talk_active);
}

/* The gate of get_source_volume() in obs-source.c, on the stand-in: the mix's
 * pressed flags (set from the queued audio actions) and source->muted. Only
 * its stop time goes through the moved helper. */
static bool mix_muted(struct gate_source *source, uint64_t os_time)
{
	if (source->push_to_mute_enabled && source->push_to_mute_pressed)
		source->push_to_mute_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_mute_delay);

	if (source->push_to_talk_enabled && source->push_to_talk_pressed)
		source->push_to_talk_stop_time = obs_source_ptm_gate_stop_time(os_time, source->push_to_talk_delay);

	bool push_to_mute_active = source->push_to_mute_pressed || os_time < source->push_to_mute_stop_time;
	bool push_to_talk_active = source->push_to_talk_pressed || os_time < source->push_to_talk_stop_time;

	return !source->enabled || source->muted || (source->push_to_mute_enabled && push_to_mute_active) ||
	       (source->push_to_talk_enabled && !push_to_talk_active);
}

#define MUTED(source, os_time, expected) check_bool(source_muted((source), (os_time)), (expected), "muted", __LINE__)

static struct gate_source enabled_source(void)
{
	struct gate_source s = {0};
	s.enabled = true;
	return s;
}

/* -------------------------------------------------------------------------
 * obs_source_ptm_gate_stop_time() */

static void stop_time_valid_delays(void **state)
{
	(void)state;

	STOP(T, 300, T + 300 * MS);
	STOP(T, 1, T + MS);
	STOP(0, 300, 300 * MS);
	/* the settings dialog's largest delay, INT_MAX ms */
	STOP(T, 2147483647ULL, T + 2147483647ULL * MS);
}

static void stop_time_delay_zero(void **state)
{
	(void)state;

	STOP(T, 0, T);
	STOP(0, 0, 0);
	STOP(1, 0, 1);
	/* nothing to add, so nothing saturates */
	STOP(UINT64_MAX, 0, UINT64_MAX);
	STOP(UINT64_MAX - 1, 0, UINT64_MAX - 1);
}

/* The product: MAX_DELAY ms is the last delay that fits; one more saturates
 * (before #2058 its product wrapped to 448384 ns, so the key let go at
 * once). */
static void stop_time_product_edge(void **state)
{
	(void)state;

	check_u64(MAX_DELAY, 18446744073709ULL, "MAX_DELAY", __LINE__);
	check_u64(MAX_DELAY_SLACK, 551615ULL, "MAX_DELAY_SLACK", __LINE__);

	STOP(0, MAX_DELAY, MAX_DELAY * MS);
	STOP(0, MAX_DELAY - 1, (MAX_DELAY - 1) * MS);
	STOP(0, MAX_DELAY + 1, UINT64_MAX);
	STOP(T, MAX_DELAY + 1, UINT64_MAX);
	STOP(0, UINT64_MAX, UINT64_MAX);
	STOP(0, UINT64_MAX / 2, UINT64_MAX);
	/* INT64_MAX, the largest delay a scene collection stores */
	STOP(0, 9223372036854775807ULL, UINT64_MAX);
	/* a negative int64 delay converted, as the setters still accept */
	STOP(T, (uint64_t)-1, UINT64_MAX);
	STOP(T, (uint64_t)-300, UINT64_MAX);
}

/* The sum: MAX_DELAY ms plus a clock of up to MAX_DELAY_SLACK fits, exactly
 * reaching UINT64_MAX at the slack; one nanosecond more saturates at the
 * same value. */
static void stop_time_sum_edge(void **state)
{
	(void)state;

	STOP(MAX_DELAY_SLACK - 1, MAX_DELAY, UINT64_MAX - 1);
	STOP(MAX_DELAY_SLACK, MAX_DELAY, UINT64_MAX);
	STOP(MAX_DELAY_SLACK + 1, MAX_DELAY, UINT64_MAX);
	STOP(T, MAX_DELAY, UINT64_MAX);
	STOP(UINT64_MAX, MAX_DELAY, UINT64_MAX);

	/* a clock near the end with a small delay */
	STOP(UINT64_MAX - MS, 1, UINT64_MAX);
	STOP(UINT64_MAX - MS - 1, 1, UINT64_MAX - 1);
	STOP(UINT64_MAX - MS + 1, 1, UINT64_MAX);
	STOP(UINT64_MAX, 1, UINT64_MAX);
	STOP(UINT64_MAX - 300 * MS, 300, UINT64_MAX);
	STOP(UINT64_MAX - 300 * MS - 1, 300, UINT64_MAX - 1);
	STOP(UINT64_MAX - 300 * MS + 1, 300, UINT64_MAX);
}

/* -------------------------------------------------------------------------
 * obs_source_ptm_gate_held() and the two gates */

static void held_before_the_stop_time(void **state)
{
	(void)state;

	HELD(T - 1, T, true);
	HELD(T, T, false);
	HELD(T + 1, T, false);
	HELD(0, 0, false);
	HELD(0, 1, true);
	HELD(UINT64_MAX - 1, UINT64_MAX, true);
	HELD(UINT64_MAX, UINT64_MAX, false);
	/* not modular: a stop time of 0 (none stored yet) is before every
	 * clock */
	HELD(UINT64_MAX, 0, false);
}

static void gates_truth_tables(void **state)
{
	(void)state;

	for (int enabled = 0; enabled < 2; enabled++) {
		for (int active = 0; active < 2; active++) {
			check_bool(obs_source_ptm_gate_mutes(enabled, active), enabled && active, "push-to-mute",
				   __LINE__);
			check_bool(obs_source_ptm_gate_talk_mutes(enabled, active), enabled && !active, "push-to-talk",
				   __LINE__);
		}
	}
}

/* -------------------------------------------------------------------------
 * source_muted() */

/* Every combination of enabled, user_muted and each gate's enabled flag and
 * key, with no stop time: muted when disabled, user-muted, push-to-mute
 * enabled and pressed, or push-to-talk enabled and not pressed. A pressed
 * key of an enabled gate stores its stop time, and nothing else does. */
static void source_muted_every_flag(void **state)
{
	(void)state;

	for (unsigned bits = 0; bits < 64; bits++) {
		struct gate_source s = {0};
		s.enabled = bits & 1;
		s.user_muted = bits & 2;
		s.push_to_mute_enabled = bits & 4;
		s.user_push_to_mute_pressed = bits & 8;
		s.push_to_talk_enabled = bits & 16;
		s.user_push_to_talk_pressed = bits & 32;
		s.push_to_mute_delay = 300;
		s.push_to_talk_delay = 500;

		bool expected = !s.enabled || s.user_muted || (s.push_to_mute_enabled && s.user_push_to_mute_pressed) ||
				(s.push_to_talk_enabled && !s.user_push_to_talk_pressed);
		bool muted = source_muted(&s, T);
		if (muted != expected) {
			fprintf(stderr, "flags 0x%02x: muted is %d, expected %d\n", bits, muted, expected);
			fail();
		}
		check_u64(s.push_to_mute_stop_time,
			  s.push_to_mute_enabled && s.user_push_to_mute_pressed ? T + 300 * MS : 0, "ptm stop time",
			  __LINE__);
		check_u64(s.push_to_talk_stop_time,
			  s.push_to_talk_enabled && s.user_push_to_talk_pressed ? T + 500 * MS : 0, "ptt stop time",
			  __LINE__);
	}
}

/* Disabled or user-muted mutes whatever the gates say, and still updates
 * the stop times. */
static void source_muted_disabled_and_user_muted(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_talk_enabled = true;
	s.user_push_to_talk_pressed = true;
	s.push_to_talk_delay = 300;
	MUTED(&s, T, false);

	s.enabled = false;
	MUTED(&s, T + SEC, true);
	check_u64(s.push_to_talk_stop_time, T + SEC + 300 * MS, "ptt stop time", __LINE__);

	s.enabled = true;
	s.user_muted = true;
	MUTED(&s, T + 2 * SEC, true);
	check_u64(s.push_to_talk_stop_time, T + 2 * SEC + 300 * MS, "ptt stop time", __LINE__);

	s.user_muted = false;
	MUTED(&s, T + 3 * SEC, false);
}

/* Push-to-talk released: open until the delay has passed, closed from the
 * stop time on. */
static void push_to_talk_release_inside_and_after_the_delay(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_talk_enabled = true;
	s.push_to_talk_delay = 300;

	/* never pressed: closed */
	MUTED(&s, T - SEC, true);

	s.user_push_to_talk_pressed = true;
	MUTED(&s, T, false);
	check_u64(s.push_to_talk_stop_time, T + 300 * MS, "ptt stop time", __LINE__);

	s.user_push_to_talk_pressed = false;
	MUTED(&s, T + 1, false);
	MUTED(&s, T + 150 * MS, false);
	MUTED(&s, T + 300 * MS - 1, false);
	MUTED(&s, T + 300 * MS, true);
	MUTED(&s, T + 300 * MS + 1, true);
	MUTED(&s, T + 10 * SEC, true);
	/* a release does not move the stop time */
	check_u64(s.push_to_talk_stop_time, T + 300 * MS, "ptt stop time", __LINE__);

	/* each packet while held moves it on */
	s.user_push_to_talk_pressed = true;
	MUTED(&s, T + 20 * SEC, false);
	MUTED(&s, T + 21 * SEC, false);
	check_u64(s.push_to_talk_stop_time, T + 21 * SEC + 300 * MS, "ptt stop time", __LINE__);
	s.user_push_to_talk_pressed = false;
	MUTED(&s, T + 21 * SEC + 300 * MS - 1, false);
	MUTED(&s, T + 21 * SEC + 300 * MS, true);
}

/* Push-to-mute released: muted until the delay has passed. */
static void push_to_mute_release_inside_and_after_the_delay(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_mute_enabled = true;
	s.push_to_mute_delay = 300;

	MUTED(&s, T - SEC, false);

	s.user_push_to_mute_pressed = true;
	MUTED(&s, T, true);
	check_u64(s.push_to_mute_stop_time, T + 300 * MS, "ptm stop time", __LINE__);

	s.user_push_to_mute_pressed = false;
	MUTED(&s, T + 1, true);
	MUTED(&s, T + 300 * MS - 1, true);
	MUTED(&s, T + 300 * MS, false);
	MUTED(&s, T + 10 * SEC, false);
}

/* A delay of 0: the stop time is the last packet's own time, so the next
 * packet after the release, even at the same time, is past it. */
static void delay_zero(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_talk_enabled = true;
	s.push_to_mute_enabled = true;

	s.user_push_to_talk_pressed = true;
	MUTED(&s, T, false);
	check_u64(s.push_to_talk_stop_time, T, "ptt stop time", __LINE__);
	s.user_push_to_talk_pressed = false;
	MUTED(&s, T, true);
	MUTED(&s, T - 1, false);

	s.push_to_talk_enabled = false;
	s.user_push_to_mute_pressed = true;
	MUTED(&s, T + SEC, true);
	check_u64(s.push_to_mute_stop_time, T + SEC, "ptm stop time", __LINE__);
	s.user_push_to_mute_pressed = false;
	MUTED(&s, T + SEC, false);
}

/* Delays at the saturation edge, through source_muted(): the key holds up to
 * the saturated stop time, not to a wrapped one. */
static void saturated_delays_hold(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_talk_enabled = true;
	s.push_to_talk_delay = MAX_DELAY + 1;
	s.user_push_to_talk_pressed = true;
	MUTED(&s, T, false);
	check_u64(s.push_to_talk_stop_time, UINT64_MAX, "ptt stop time", __LINE__);
	s.user_push_to_talk_pressed = false;
	MUTED(&s, T + 1, false);
	MUTED(&s, UINT64_MAX - 1, false);
	/* characterized, not endorsed: UINT64_MAX itself is not before it */
	MUTED(&s, UINT64_MAX, true);

	/* the sum: MAX_DELAY from a clock past the slack */
	s.push_to_talk_delay = MAX_DELAY;
	s.user_push_to_talk_pressed = true;
	MUTED(&s, MAX_DELAY_SLACK + 1, false);
	check_u64(s.push_to_talk_stop_time, UINT64_MAX, "ptt stop time", __LINE__);
	s.user_push_to_talk_pressed = false;
	MUTED(&s, MAX_DELAY_SLACK + 2, false);

	/* just inside: no saturation, the exact sum */
	s.user_push_to_talk_pressed = true;
	MUTED(&s, MAX_DELAY_SLACK - 1, false);
	check_u64(s.push_to_talk_stop_time, UINT64_MAX - 1, "ptt stop time", __LINE__);
	s.user_push_to_talk_pressed = false;
	MUTED(&s, UINT64_MAX - 2, false);
	MUTED(&s, UINT64_MAX - 1, true);

	struct gate_source m = enabled_source();
	m.push_to_mute_enabled = true;
	m.push_to_mute_delay = UINT64_MAX;
	m.user_push_to_mute_pressed = true;
	MUTED(&m, T, true);
	check_u64(m.push_to_mute_stop_time, UINT64_MAX, "ptm stop time", __LINE__);
	m.user_push_to_mute_pressed = false;
	MUTED(&m, T + 100 * SEC, true);
}

/* A pressed key of a disabled gate stores nothing, and the gate does not
 * mute. Characterized, not endorsed: a stop time stored before the gate was
 * turned off still holds when it is turned back on. */
static void disabled_gate_stores_nothing(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_mute_delay = 300;
	s.user_push_to_mute_pressed = true;
	MUTED(&s, T, false);
	check_u64(s.push_to_mute_stop_time, 0, "ptm stop time", __LINE__);

	s.push_to_mute_enabled = true;
	MUTED(&s, T + SEC, true);
	check_u64(s.push_to_mute_stop_time, T + SEC + 300 * MS, "ptm stop time", __LINE__);
	s.user_push_to_mute_pressed = false;
	s.push_to_mute_enabled = false;
	MUTED(&s, T + SEC + 1, false);
	s.push_to_mute_enabled = true;
	MUTED(&s, T + SEC + 2, true);
	MUTED(&s, T + SEC + 300 * MS, false);
}

/* Characterized, not endorsed: while held, every packet stores its own time
 * plus the delay, so a packet earlier than the last one (os_time is
 * os_gettime_ns() for source_muted(), an audio timestamp for the mix) moves
 * the stop time back. */
static void stop_time_follows_the_last_packet(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_talk_enabled = true;
	s.push_to_talk_delay = 300;
	s.user_push_to_talk_pressed = true;
	MUTED(&s, T + 10 * SEC, false);
	MUTED(&s, T, false);
	check_u64(s.push_to_talk_stop_time, T + 300 * MS, "ptt stop time", __LINE__);
	s.user_push_to_talk_pressed = false;
	MUTED(&s, T + 300 * MS, true);
}

/* Both gates on one source: push-to-mute wins while active, and
 * push-to-talk still has to be held. */
static void both_gates(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_mute_enabled = true;
	s.push_to_talk_enabled = true;
	s.push_to_mute_delay = 100;
	s.push_to_talk_delay = 300;

	MUTED(&s, T, true);
	s.user_push_to_talk_pressed = true;
	MUTED(&s, T, false);
	s.user_push_to_mute_pressed = true;
	MUTED(&s, T + MS, true);
	s.user_push_to_mute_pressed = false;
	MUTED(&s, T + 101 * MS - 1, true);
	MUTED(&s, T + 101 * MS, false);
	s.user_push_to_talk_pressed = false;
	check_u64(s.push_to_talk_stop_time, T + 101 * MS + 300 * MS, "ptt stop time", __LINE__);
	MUTED(&s, T + 401 * MS - 1, false);
	MUTED(&s, T + 401 * MS, true);
}

/* get_source_volume() and source_muted() share the stop times: each keeps
 * its own pressed flags (the mix's from the queued audio actions, the
 * capture callbacks' from the hotkey thread), but a stop time either stores
 * is the one both read. */
static void mix_and_capture_share_the_stop_time(void **state)
{
	(void)state;

	struct gate_source s = enabled_source();
	s.push_to_talk_enabled = true;
	s.push_to_talk_delay = 300;

	/* the mix's action says held; the hotkey thread has already let go */
	s.push_to_talk_pressed = true;
	check_bool(mix_muted(&s, T), false, "mix muted", __LINE__);
	check_u64(s.push_to_talk_stop_time, T + 300 * MS, "ptt stop time", __LINE__);
	MUTED(&s, T + 300 * MS - 1, false);
	MUTED(&s, T + 300 * MS, true);

	/* and the other way: a capture packet's store holds the mix open */
	s.push_to_talk_pressed = false;
	s.user_push_to_talk_pressed = true;
	MUTED(&s, T + SEC, false);
	s.user_push_to_talk_pressed = false;
	check_bool(mix_muted(&s, T + SEC + 300 * MS - 1), false, "mix muted", __LINE__);
	check_bool(mix_muted(&s, T + SEC + 300 * MS), true, "mix muted", __LINE__);

	/* the mix saturates through the same helper */
	s.push_to_talk_delay = UINT64_MAX;
	s.push_to_talk_pressed = true;
	check_bool(mix_muted(&s, T + 2 * SEC), false, "mix muted", __LINE__);
	check_u64(s.push_to_talk_stop_time, UINT64_MAX, "ptt stop time", __LINE__);

	/* the mix reads source->muted, not user_muted */
	s.user_muted = true;
	check_bool(mix_muted(&s, T + 3 * SEC), false, "mix muted", __LINE__);
	MUTED(&s, T + 3 * SEC, true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(stop_time_valid_delays),
		cmocka_unit_test(stop_time_delay_zero),
		cmocka_unit_test(stop_time_product_edge),
		cmocka_unit_test(stop_time_sum_edge),
		cmocka_unit_test(held_before_the_stop_time),
		cmocka_unit_test(gates_truth_tables),
		cmocka_unit_test(source_muted_every_flag),
		cmocka_unit_test(source_muted_disabled_and_user_muted),
		cmocka_unit_test(push_to_talk_release_inside_and_after_the_delay),
		cmocka_unit_test(push_to_mute_release_inside_and_after_the_delay),
		cmocka_unit_test(delay_zero),
		cmocka_unit_test(saturated_delays_hold),
		cmocka_unit_test(disabled_gate_stores_nothing),
		cmocka_unit_test(stop_time_follows_the_last_packet),
		cmocka_unit_test(both_gates),
		cmocka_unit_test(mix_and_capture_share_the_stop_time),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
