#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "pulseaudio-monitor-steps.h"
#include "pulseaudio-stream-flags.h"

/* Characterization of the PulseAudio monitor's open and write steps
 * (pulseaudio-monitor-steps.c), moved out of pulseaudio-output.c before
 * their Rust port: do_stream_write()'s grow, cork and write rules,
 * on_audio_playback()'s skip, byte count and volume rules, pulseaudio_sink_info()'s
 * results, audio_monitor_init()'s device, feedback, fallback, format and
 * buffer rules, on_sink_event()'s filter, audio_monitor_reset()'s context
 * rule and audio_monitor_destroy()'s last monitor. No server: the test calls
 * only the moved functions, and stand-ins below put them together in the
 * order pulseaudio-output.c asks them.
 *
 * Characterized, not endorsed:
 * - tlength * 2 is a uint32_t product, so a tlength of 2^31 or more wraps
 *   and the buffer grows for a much smaller queue (any queue at all for
 *   exactly 2^31);
 * - the new tlength is the queue truncated to uint32_t, so a queue past
 *   UINT32_MAX bytes shrinks the buffer instead (2^32 + 5 bytes give 5);
 * - a NaN volume is not close to 1, so the volume step runs with it;
 * - negative activate refs are not idle;
 * - an empty monitoring device ("") is a named device: it falls back to the
 *   default sink when it gives no format. */

#define ARRAY_SIZE(a) (sizeof(a) / sizeof((a)[0]))

#define CHECK(cond, ...)                              \
	do {                                          \
		if (!(cond)) {                        \
			fprintf(stderr, __VA_ARGS__); \
			fprintf(stderr, "\n");        \
			fail();                       \
		}                                     \
	} while (0)

static void copied_constants(void **state)
{
	(void)state;
	assert_int_equal(PULSEAUDIO_MONITOR_DO_NOT_SELF_MONITOR, 1 << 9);
	assert_int_equal(PULSEAUDIO_MONITOR_SINK_INFO, 0);
	assert_int_equal(PULSEAUDIO_MONITOR_SINK_END, 1);
	assert_int_equal(PULSEAUDIO_MONITOR_SINK_ERROR, 2);
}

/* ---- do_stream_write() ---- */

static void grow_is_strictly_past_twice_tlength(void **state)
{
	(void)state;
	static const uint32_t tl[] = {1, 2, 4800, 9600, 1000000, 0x7fffffffu};
	for (size_t i = 0; i < ARRAY_SIZE(tl); i++) {
		size_t twice = (size_t)tl[i] * 2;
		CHECK(!pulseaudio_monitor_grow(twice - 1, tl[i]), "tlength %u: 2 * tlength - 1 grew", tl[i]);
		CHECK(!pulseaudio_monitor_grow(twice, tl[i]), "tlength %u: 2 * tlength grew (the test is >)", tl[i]);
		CHECK(pulseaudio_monitor_grow(twice + 1, tl[i]), "tlength %u: 2 * tlength + 1 did not grow", tl[i]);
	}
	assert_false(pulseaudio_monitor_grow(0, 0));
	assert_true(pulseaudio_monitor_grow(1, 0));
	assert_false(pulseaudio_monitor_grow(0, 4800));
	assert_true(pulseaudio_monitor_grow(SIZE_MAX, 4800));
}

static void grow_product_wraps_at_2_31(void **state)
{
	(void)state;
	/* Characterized, not endorsed: tlength * 2 wraps in uint32_t. */
	assert_true(pulseaudio_monitor_grow(1, 0x80000000u));
	assert_false(pulseaudio_monitor_grow(0, 0x80000000u));
	assert_false(pulseaudio_monitor_grow(2, 0x80000001u));
	assert_true(pulseaudio_monitor_grow(3, 0x80000001u));
	assert_false(pulseaudio_monitor_grow(0xfffffffeu, 0xffffffffu));
	assert_true(pulseaudio_monitor_grow(0xffffffffu, 0xffffffffu));
	if (SIZE_MAX > UINT32_MAX) {
		/* Not wrapped, it would need more than 2^32 bytes. */
		assert_true(pulseaudio_monitor_grow((size_t)0xffffffffu + 1, 0x80000000u));
	}
}

static void grown_tlength_truncates(void **state)
{
	(void)state;
	static const size_t q[] = {0, 1, 9601, 0x7fffffff, 0xffffffffu};
	for (size_t i = 0; i < ARRAY_SIZE(q); i++)
		assert_int_equal(pulseaudio_monitor_grown_tlength(q[i]), (uint32_t)q[i]);
	if (SIZE_MAX > UINT32_MAX) {
		/* Characterized, not endorsed: past UINT32_MAX the buffer
		 * shrinks. */
		size_t big = (size_t)0xffffffffu + 6;
		assert_int_equal(pulseaudio_monitor_grown_tlength(big), 5);
		assert_int_equal(pulseaudio_monitor_grown_tlength((size_t)0xffffffffu + 1), 0);
		assert_int_equal(pulseaudio_monitor_grown_tlength(SIZE_MAX), 0xffffffffu);
	}
}

static void uncork_at_a_whole_tlength(void **state)
{
	(void)state;
	static const uint32_t tl[] = {1, 4800, 9600, 0x7fffffffu, 0xffffffffu};
	for (size_t i = 0; i < ARRAY_SIZE(tl); i++) {
		CHECK(!pulseaudio_monitor_uncork((size_t)tl[i] - 1, tl[i]), "tlength %u: uncorked one byte short",
		      tl[i]);
		CHECK(pulseaudio_monitor_uncork(tl[i], tl[i]), "tlength %u: stayed corked at a whole tlength", tl[i]);
		CHECK(pulseaudio_monitor_uncork((size_t)tl[i] + 1, tl[i]), "tlength %u: stayed corked past it", tl[i]);
	}
	/* A tlength of 0 uncorks at once, even with nothing queued. */
	assert_true(pulseaudio_monitor_uncork(0, 0));
}

static void write_loop_and_short_write(void **state)
{
	(void)state;
	assert_false(pulseaudio_monitor_write_more(0));
	assert_true(pulseaudio_monitor_write_more(1));
	assert_true(pulseaudio_monitor_write_more(SIZE_MAX));

	static const size_t q[] = {1, 4800, 9600, SIZE_MAX - 1};
	for (size_t i = 0; i < ARRAY_SIZE(q); i++) {
		CHECK(pulseaudio_monitor_write_short(q[i] + 1, q[i]), "queue %zu: asked for more and wrote", q[i]);
		CHECK(!pulseaudio_monitor_write_short(q[i], q[i]), "queue %zu: asked for all and cancelled", q[i]);
		CHECK(!pulseaudio_monitor_write_short(q[i] - 1, q[i]), "queue %zu: asked for less and cancelled", q[i]);
	}
	assert_false(pulseaudio_monitor_write_short(0, 0));
	assert_true(pulseaudio_monitor_write_short(SIZE_MAX, 0));
}

/* do_stream_write() put together as pulseaudio-output.c does, with the
 * stream's calls replaced by what the server asks. */
struct stand_in_write {
	size_t queued;
	uint32_t tlength;
	bool corked;
	/* results */
	int grew;
	bool uncorked;
	size_t written;
	int writes;
	bool cancelled;
};

static void stand_in_write(struct stand_in_write *w, const size_t *asks, size_t n_asks)
{
	if (pulseaudio_monitor_grow(w->queued, w->tlength)) {
		w->tlength = pulseaudio_monitor_grown_tlength(w->queued);
		w->grew++;
	}
	if (w->corked) {
		if (!pulseaudio_monitor_uncork(w->queued, w->tlength))
			return;
		w->corked = false;
		w->uncorked = true;
	}
	size_t k = 0;
	while (pulseaudio_monitor_write_more(w->queued)) {
		if (k == n_asks)
			return; /* pa_stream_begin_write() failed */
		size_t ask = asks[k++];
		if (pulseaudio_monitor_write_short(ask, w->queued)) {
			w->cancelled = true;
			return;
		}
		w->queued -= ask;
		w->written += ask;
		w->writes++;
	}
}

static void write_end_to_end(void **state)
{
	(void)state;
	/* 25 ms at 48 kHz stereo float: 9600 bytes. */
	const uint32_t tl = 9600;

	/* Corked, one byte short: nothing is written, the buffer stays. */
	struct stand_in_write a = {.queued = tl - 1, .tlength = tl, .corked = true};
	size_t ask_all[] = {tl - 1};
	stand_in_write(&a, ask_all, 1);
	assert_false(a.uncorked);
	assert_int_equal(a.written, 0);
	assert_int_equal(a.tlength, tl);

	/* Corked, a whole tlength: uncorked and written in one go. */
	struct stand_in_write b = {.queued = tl, .tlength = tl, .corked = true};
	size_t ask_b[] = {tl};
	stand_in_write(&b, ask_b, 1);
	assert_true(b.uncorked);
	assert_int_equal(b.written, tl);
	assert_int_equal(b.queued, 0);

	/* The server asks for more than is queued: cancel and wait, nothing
	 * popped. */
	struct stand_in_write c = {.queued = 4000, .tlength = tl};
	size_t ask_c[] = {4001};
	stand_in_write(&c, ask_c, 1);
	assert_true(c.cancelled);
	assert_int_equal(c.written, 0);
	assert_int_equal(c.queued, 4000);

	/* Asked in pieces: written until the queue is empty. */
	struct stand_in_write d = {.queued = 5000, .tlength = tl};
	size_t ask_d[] = {2000, 2000, 1000};
	stand_in_write(&d, ask_d, 3);
	assert_int_equal(d.writes, 3);
	assert_int_equal(d.written, 5000);
	assert_false(d.cancelled);

	/* A piece, then more than is left: two writes are kept, the rest
	 * waits. */
	struct stand_in_write e = {.queued = 5000, .tlength = tl};
	size_t ask_e[] = {3000, 2001};
	stand_in_write(&e, ask_e, 2);
	assert_int_equal(e.writes, 1);
	assert_int_equal(e.queued, 2000);
	assert_true(e.cancelled);

	/* Past twice tlength: the buffer grows to the queue, which then
	 * uncorks a corked stream. */
	struct stand_in_write f = {.queued = 2 * (size_t)tl + 1, .tlength = tl, .corked = true};
	size_t ask_f[] = {2 * (size_t)tl + 1};
	stand_in_write(&f, ask_f, 1);
	assert_int_equal(f.grew, 1);
	assert_int_equal(f.tlength, 2 * tl + 1);
	assert_true(f.uncorked);
	assert_int_equal(f.written, 2 * (size_t)tl + 1);

	/* At exactly twice tlength it does not grow. */
	struct stand_in_write g = {.queued = 2 * (size_t)tl, .tlength = tl, .corked = true};
	stand_in_write(&g, NULL, 0);
	assert_int_equal(g.grew, 0);
	assert_int_equal(g.tlength, tl);
	assert_true(g.uncorked);
}

/* ---- on_audio_playback() ---- */

static void playback_skips_inactive_sources(void **state)
{
	(void)state;
	assert_true(pulseaudio_monitor_playback_idle(0));
	assert_false(pulseaudio_monitor_playback_idle(1));
	assert_false(pulseaudio_monitor_playback_idle(2));
	assert_false(pulseaudio_monitor_playback_idle(LONG_MAX));
	/* Characterized, not endorsed: a negative count is not idle. */
	assert_false(pulseaudio_monitor_playback_idle(-1));
	assert_false(pulseaudio_monitor_playback_idle(LONG_MIN));
}

static void playback_bytes(void **state)
{
	(void)state;
	assert_int_equal(pulseaudio_monitor_playback_bytes(8, 480), 3840);
	assert_int_equal(pulseaudio_monitor_playback_bytes(8, 0), 0);
	assert_int_equal(pulseaudio_monitor_playback_bytes(0, 480), 0);
	assert_int_equal(pulseaudio_monitor_playback_bytes(1, 0xffffffffu), 0xffffffffu);
	/* A product in uint_fast32_t: 64 bits on glibc, 32 on FreeBSD. */
	size_t big = pulseaudio_monitor_playback_bytes(32, 0x10000000u);
	if (sizeof(uint_fast32_t) >= 8 && SIZE_MAX > UINT32_MAX)
		assert_true(big == (size_t)32 * 0x10000000u);
	else
		assert_int_equal(big, 0);
}

static void volume_step_unless_close_to_one(void **state)
{
	(void)state;
	assert_false(pulseaudio_monitor_needs_volume(1.0f));
	/* The last floats within EPSILON (1e-4f) of 1, and the next ones. */
	assert_false(pulseaudio_monitor_needs_volume(0x1.00068cp+0f));
	assert_true(pulseaudio_monitor_needs_volume(0x1.00068ep+0f));
	assert_false(pulseaudio_monitor_needs_volume(0x1.fff2e6p-1f));
	assert_true(pulseaudio_monitor_needs_volume(0x1.fff2e4p-1f));
	/* 1 + EPSILON and 1 - EPSILON round past it in float. */
	assert_true(pulseaudio_monitor_needs_volume(1.0f + 1e-4f));
	assert_true(pulseaudio_monitor_needs_volume(1.0f - 1e-4f));
	assert_false(pulseaudio_monitor_needs_volume(1.00005f));
	assert_false(pulseaudio_monitor_needs_volume(0.99995f));
	assert_true(pulseaudio_monitor_needs_volume(0.0f));
	assert_true(pulseaudio_monitor_needs_volume(-0.0f));
	assert_true(pulseaudio_monitor_needs_volume(0.5f));
	assert_true(pulseaudio_monitor_needs_volume(2.0f));
	assert_true(pulseaudio_monitor_needs_volume(-1.0f));
	assert_true(pulseaudio_monitor_needs_volume(INFINITY));
	assert_true(pulseaudio_monitor_needs_volume(-INFINITY));
	/* Characterized, not endorsed: NaN is processed. */
	assert_true(pulseaudio_monitor_needs_volume(NAN));
}

/* ---- pulseaudio_sink_info() ---- */

static void sink_info_results(void **state)
{
	(void)state;
	assert_int_equal(pulseaudio_monitor_sink_result(0), PULSEAUDIO_MONITOR_SINK_INFO);
	static const int ends[] = {1, 2, 1000, INT_MAX};
	for (size_t i = 0; i < ARRAY_SIZE(ends); i++)
		assert_int_equal(pulseaudio_monitor_sink_result(ends[i]), PULSEAUDIO_MONITOR_SINK_END);
	static const int errors[] = {-1, -2, -1000, INT_MIN};
	for (size_t i = 0; i < ARRAY_SIZE(errors); i++)
		assert_int_equal(pulseaudio_monitor_sink_result(errors[i]), PULSEAUDIO_MONITOR_SINK_ERROR);
}

/* ---- audio_monitor_init() ---- */

static void device_and_default(void **state)
{
	(void)state;
	assert_false(pulseaudio_monitor_has_device(NULL));
	assert_true(pulseaudio_monitor_has_device(""));
	assert_true(pulseaudio_monitor_has_device("default"));

	assert_true(pulseaudio_monitor_is_default("default"));
	static const char *not_default[] = {"", "Default", "default ", "defaults", "default.monitor", "sink"};
	for (size_t i = 0; i < ARRAY_SIZE(not_default); i++)
		CHECK(!pulseaudio_monitor_is_default(not_default[i]), "'%s' taken as default", not_default[i]);
}

static void self_monitor_flag(void **state)
{
	(void)state;
	for (uint32_t bit = 0; bit < 32; bit++) {
		uint32_t flags = 1u << bit;
		CHECK(pulseaudio_monitor_self_monitor_check(flags) == (bit == 9), "flag bit %u", bit);
		CHECK(pulseaudio_monitor_self_monitor_check(~flags) == (bit != 9), "flags without bit %u", bit);
	}
	assert_false(pulseaudio_monitor_self_monitor_check(0));
	assert_true(pulseaudio_monitor_self_monitor_check(UINT32_MAX));
}

static void fallback_rule(void **state)
{
	(void)state;
	static const char *named[] = {"sink.a", "alsa_output.usb", "defaults", "Default", ""};
	for (size_t i = 0; i < ARRAY_SIZE(named); i++) {
		CHECK(pulseaudio_monitor_fallback(PA_SAMPLE_INVALID, named[i]), "'%s' gone: no fallback", named[i]);
		CHECK(!pulseaudio_monitor_fallback(PA_SAMPLE_FLOAT32LE, named[i]), "'%s' present: fell back", named[i]);
		CHECK(!pulseaudio_monitor_fallback(PA_SAMPLE_S16LE, named[i]), "'%s' present: fell back", named[i]);
	}
	assert_false(pulseaudio_monitor_fallback(PA_SAMPLE_INVALID, "default"));
	assert_false(pulseaudio_monitor_fallback(PA_SAMPLE_U8, "default"));
	assert_false(pulseaudio_monitor_fallback(PA_SAMPLE_MAX, "sink.a"));

	assert_false(pulseaudio_monitor_sink_named(NULL));
	assert_false(pulseaudio_monitor_sink_named(""));
	assert_true(pulseaudio_monitor_sink_named("x"));
	assert_true(pulseaudio_monitor_sink_named("default"));

	assert_true(pulseaudio_monitor_format_invalid(PA_SAMPLE_INVALID));
	for (int f = 0; f <= PA_SAMPLE_MAX; f++)
		CHECK(!pulseaudio_monitor_format_invalid((pa_sample_format_t)f), "format %d taken as invalid", f);
	assert_false(pulseaudio_monitor_format_invalid((pa_sample_format_t)-2));
}

static void buffer_and_flags(void **state)
{
	(void)state;
	assert_true(pulseaudio_monitor_tlength_usec() == 25000);

	static const char *ids[] = {"default", "sink.a", "", NULL};
	for (size_t i = 0; i < ARRAY_SIZE(ids); i++) {
		const char *id = ids[i] ? ids[i] : "(null)";
		CHECK(pulseaudio_monitor_open_flags(true, ids[i]) == pulseaudio_monitor_stream_flags("default"),
		      "fallback stream for '%s' does not have the flags of default", id);
		CHECK(pulseaudio_monitor_open_flags(false, ids[i]) == pulseaudio_monitor_stream_flags(ids[i]),
		      "stream for '%s' does not have its own flags", id);
	}
	assert_false(pulseaudio_monitor_open_flags(true, "sink.a") & PA_STREAM_DONT_MOVE);
	assert_true(pulseaudio_monitor_open_flags(false, "sink.a") & PA_STREAM_DONT_MOVE);
	assert_false(pulseaudio_monitor_open_flags(false, "default") & PA_STREAM_DONT_MOVE);
}

/* audio_monitor_init()'s decisions put together in its order, with the
 * calls replaced by their answers. */
enum route { ROUTE_FAIL, ROUTE_IGNORE, ROUTE_DEFAULT, ROUTE_NAMED, ROUTE_FALLBACK };

struct open_in {
	const char *id;
	uint32_t output_flags;
	bool match;             /* devices_match() */
	pa_sample_format_t fmt; /* after the first sink info */
	const char *default_id; /* get_default_id() on the fallback */
	pa_sample_format_t fallback_fmt;
};

static enum route stand_in_open(const struct open_in *in, pa_stream_flags_t *flags)
{
	if (!pulseaudio_monitor_has_device(in->id))
		return ROUTE_FAIL;
	if (pulseaudio_monitor_self_monitor_check(in->output_flags) && in->match)
		return ROUTE_IGNORE;
	bool is_default = pulseaudio_monitor_is_default(in->id);
	bool fallback = false;
	pa_sample_format_t fmt = in->fmt;
	if (pulseaudio_monitor_fallback(fmt, in->id)) {
		fallback = true;
		if (!pulseaudio_monitor_sink_named(in->default_id))
			return ROUTE_FAIL;
		fmt = in->fallback_fmt;
	}
	if (pulseaudio_monitor_format_invalid(fmt))
		return ROUTE_FAIL;
	*flags = pulseaudio_monitor_open_flags(fallback, in->id);
	return fallback ? ROUTE_FALLBACK : is_default ? ROUTE_DEFAULT : ROUTE_NAMED;
}

static void open_routes(void **state)
{
	(void)state;
	const uint32_t self = PULSEAUDIO_MONITOR_DO_NOT_SELF_MONITOR;
	const pa_sample_format_t F = PA_SAMPLE_FLOAT32LE, I = PA_SAMPLE_INVALID;
	struct {
		struct open_in in;
		enum route route;
		bool dont_move;
	} cases[] = {
		{{NULL, 0, false, F, "s", F}, ROUTE_FAIL, false},
		{{NULL, self, true, F, "s", F}, ROUTE_FAIL, false},
		{{"default", self, true, F, "s", F}, ROUTE_IGNORE, false},
		{{"sink.a", self, true, F, "s", F}, ROUTE_IGNORE, false},
		{{"sink.a", 0, true, F, "s", F}, ROUTE_NAMED, true},
		{{"sink.a", self, false, F, "s", F}, ROUTE_NAMED, true},
		{{"default", 0, false, F, "s", F}, ROUTE_DEFAULT, false},
		{{"default", 0, false, I, "s", F}, ROUTE_FAIL, false},
		{{"sink.a", 0, false, I, "s", F}, ROUTE_FALLBACK, false},
		{{"sink.a", 0, false, I, "s", I}, ROUTE_FAIL, false},
		{{"sink.a", 0, false, I, "", F}, ROUTE_FAIL, false},
		{{"sink.a", 0, false, I, NULL, F}, ROUTE_FAIL, false},
		{{"", 0, false, I, "s", F}, ROUTE_FALLBACK, false},
		{{"", 0, false, F, "s", F}, ROUTE_NAMED, true},
	};
	for (size_t i = 0; i < ARRAY_SIZE(cases); i++) {
		pa_stream_flags_t flags = 0;
		enum route r = stand_in_open(&cases[i].in, &flags);
		CHECK(r == cases[i].route, "case %zu: route %d, expected %d", i, (int)r, (int)cases[i].route);
		if (r >= ROUTE_DEFAULT)
			CHECK(!!(flags & PA_STREAM_DONT_MOVE) == cases[i].dont_move, "case %zu: flags 0x%x", i,
			      (unsigned)flags);
	}
}

/* ---- on_sink_event() ---- */

static void only_new_and_remove_are_queued(void **state)
{
	(void)state;
	static const pa_subscription_event_type_t facilities[] = {
		PA_SUBSCRIPTION_EVENT_SINK,   PA_SUBSCRIPTION_EVENT_SOURCE, PA_SUBSCRIPTION_EVENT_SINK_INPUT,
		PA_SUBSCRIPTION_EVENT_SERVER, PA_SUBSCRIPTION_EVENT_CARD,   PA_SUBSCRIPTION_EVENT_FACILITY_MASK,
	};
	for (size_t i = 0; i < ARRAY_SIZE(facilities); i++) {
		pa_subscription_event_type_t f = facilities[i];
		CHECK(pulseaudio_monitor_event_queued(f | PA_SUBSCRIPTION_EVENT_NEW), "facility %u: new", (unsigned)f);
		CHECK(pulseaudio_monitor_event_queued(f | PA_SUBSCRIPTION_EVENT_REMOVE), "facility %u: remove",
		      (unsigned)f);
		CHECK(!pulseaudio_monitor_event_queued(f | PA_SUBSCRIPTION_EVENT_CHANGE), "facility %u: change",
		      (unsigned)f);
		CHECK(!pulseaudio_monitor_event_queued(f | PA_SUBSCRIPTION_EVENT_TYPE_MASK), "facility %u: type 0x30",
		      (unsigned)f);
	}
	/* Bits above the type mask are ignored. */
	assert_true(pulseaudio_monitor_event_queued((pa_subscription_event_type_t)(0x40 | PA_SUBSCRIPTION_EVENT_NEW)));
	assert_true(pulseaudio_monitor_event_queued(
		(pa_subscription_event_type_t)(0xffffff00u | PA_SUBSCRIPTION_EVENT_REMOVE)));
	assert_false(pulseaudio_monitor_event_queued(
		(pa_subscription_event_type_t)(0xffffff00u | PA_SUBSCRIPTION_EVENT_CHANGE)));
}

/* ---- audio_monitor_reset() and audio_monitor_destroy() ---- */

static void context_states(void **state)
{
	(void)state;
	assert_false(pulseaudio_monitor_context_good(PA_CONTEXT_UNCONNECTED));
	assert_true(pulseaudio_monitor_context_good(PA_CONTEXT_CONNECTING));
	assert_true(pulseaudio_monitor_context_good(PA_CONTEXT_AUTHORIZING));
	assert_true(pulseaudio_monitor_context_good(PA_CONTEXT_SETTING_NAME));
	assert_true(pulseaudio_monitor_context_good(PA_CONTEXT_READY));
	assert_false(pulseaudio_monitor_context_good(PA_CONTEXT_FAILED));
	assert_false(pulseaudio_monitor_context_good(PA_CONTEXT_TERMINATED));
	assert_false(pulseaudio_monitor_context_good((pa_context_state_t)-1));
	assert_false(pulseaudio_monitor_context_good((pa_context_state_t)7));
}

enum reset_route { RESET_REPLACE, RESET_KEEP, RESET_IGNORE };

/* audio_monitor_reset()'s branches, with the calls replaced by their
 * answers. */
static enum reset_route stand_in_reset(bool init_ok, bool has_stream, bool has_context, pa_context_state_t state)
{
	if (init_ok)
		return RESET_REPLACE;
	if (!has_stream)
		return RESET_KEEP;
	bool context_good = has_context && pulseaudio_monitor_context_good(state);
	return context_good ? RESET_KEEP : RESET_IGNORE;
}

static void reset_routes(void **state)
{
	(void)state;
	for (int s = -1; s <= 7; s++) {
		pa_context_state_t st = (pa_context_state_t)s;
		bool good = s >= PA_CONTEXT_CONNECTING && s <= PA_CONTEXT_READY;
		CHECK(stand_in_reset(true, true, true, st) == RESET_REPLACE, "state %d: init ok", s);
		CHECK(stand_in_reset(true, false, false, st) == RESET_REPLACE, "state %d: init ok, no stream", s);
		CHECK(stand_in_reset(false, false, true, st) == RESET_KEEP, "state %d: no stream", s);
		CHECK(stand_in_reset(false, true, false, st) == RESET_IGNORE, "state %d: no context", s);
		CHECK(stand_in_reset(false, true, true, st) == (good ? RESET_KEEP : RESET_IGNORE), "state %d", s);
	}
}

static void last_monitor(void **state)
{
	(void)state;
	assert_true(pulseaudio_monitor_last(0));
	assert_false(pulseaudio_monitor_last(1));
	assert_false(pulseaudio_monitor_last(2));
	assert_false(pulseaudio_monitor_last(SIZE_MAX));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(copied_constants),
		cmocka_unit_test(grow_is_strictly_past_twice_tlength),
		cmocka_unit_test(grow_product_wraps_at_2_31),
		cmocka_unit_test(grown_tlength_truncates),
		cmocka_unit_test(uncork_at_a_whole_tlength),
		cmocka_unit_test(write_loop_and_short_write),
		cmocka_unit_test(write_end_to_end),
		cmocka_unit_test(playback_skips_inactive_sources),
		cmocka_unit_test(playback_bytes),
		cmocka_unit_test(volume_step_unless_close_to_one),
		cmocka_unit_test(sink_info_results),
		cmocka_unit_test(device_and_default),
		cmocka_unit_test(self_monitor_flag),
		cmocka_unit_test(fallback_rule),
		cmocka_unit_test(buffer_and_flags),
		cmocka_unit_test(open_routes),
		cmocka_unit_test(only_new_and_remove_are_queued),
		cmocka_unit_test(context_states),
		cmocka_unit_test(reset_routes),
		cmocka_unit_test(last_monitor),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
