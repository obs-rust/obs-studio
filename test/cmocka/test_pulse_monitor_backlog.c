#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "pulseaudio-stream-flags.h"

/* Audio monitoring on Linux falls behind after any playback stall and never
 * catches up (obsproject/obs-studio#4531, #3525 and #10548; about 35
 * reporters, plus forum threads and scripts that toggle monitoring off and
 * on every few minutes).
 *
 * The PulseAudio monitor (libobs/audio-monitoring/pulse/pulseaudio-output.c,
 * also used with PipeWire) writes each packet to the server as soon as OBS
 * outputs it. When playback stalls while OBS keeps producing, the audio
 * produced meanwhile is queued and played in order afterwards, and the queue
 * drains only as fast as it fills: the monitor stays as far behind as the
 * stall lasted. With a private PipeWire paused for 3 s, the delay from a
 * volume change to hearing it went from 0.15 s to 3.3 s and stayed there.
 *
 * The decision to drop the excess is kept free of stream calls so it can be
 * tested here: pulseaudio_monitor_backlog_excess() gets how much audio is
 * queued (server buffer plus not yet written) once per write, and returns
 * how much to drop. The cases drive it with a model of the queue: a base
 * that moves the way it does in normal playback, plus a backlog that a
 * stall or clock drift adds and that only a drop removes. */

#define MS 1000ULL                 /* usec */
#define NS_PER_USEC 1000ULL        /* ns */
#define PACKET_NS 21333333ULL      /* 1024 frames at 48 kHz */
#define SECOND_NS 1000000000ULL

struct model {
	/* Queued audio without any backlog at time t (ns), in usec. */
	uint64_t (*base)(uint64_t t);
	uint64_t target;

	/* Backlog added at stall_at (0: none) */
	uint64_t stall_at;
	uint64_t stall_backlog;
	/* Backlog that drains by itself within drain_ns after stall_at (a
	 * burst rather than a stall); 0: it stays until dropped. */
	uint64_t drain_ns;
	/* Backlog added per second of playback (clock drift), in usec. */
	uint64_t drift_per_s;

	/* Results */
	uint64_t backlog;
	uint64_t dropped;
	unsigned drops;
	unsigned underruns; /* drops that would have emptied the queue */
	uint64_t max_queued_after;
	uint64_t check_after; /* ns; max_queued_after counts from here */
};

static uint64_t backlog_at(struct model *m, uint64_t t, uint64_t prev_t)
{
	if (m->drift_per_s)
		m->backlog += m->drift_per_s * (t - prev_t) / SECOND_NS;

	if (m->stall_at && prev_t < m->stall_at && t >= m->stall_at)
		m->backlog += m->stall_backlog;

	if (m->drain_ns && t >= m->stall_at && m->backlog) {
		uint64_t rate = m->stall_backlog * PACKET_NS / m->drain_ns;
		m->backlog = m->backlog > rate ? m->backlog - rate : 0;
	}
	return m->backlog;
}

static void run(struct model *m, uint64_t duration_ns)
{
	struct pulseaudio_monitor_backlog backlog = {0};
	uint64_t prev_t = 0;

	for (uint64_t t = PACKET_NS; t <= duration_ns; t += PACKET_NS) {
		uint64_t base = m->base(t);
		uint64_t queued = base + backlog_at(m, t, prev_t);
		prev_t = t;

		uint64_t drop = pulseaudio_monitor_backlog_excess(&backlog, t, queued, m->target);
		if (drop) {
			m->drops++;
			m->dropped += drop;
			if (drop >= queued)
				m->underruns++;
			uint64_t from_backlog = drop < m->backlog ? drop : m->backlog;
			m->backlog -= from_backlog;
			queued = queued > drop ? queued - drop : 0;
		}

		if (t >= m->check_after && queued > m->max_queued_after)
			m->max_queued_after = queued;
	}
}

/* Normal playback: the queue moves between 100 and 160 ms as packets arrive
 * and the server reads, with a target of 85 ms (what the monitor asks for
 * once a Media Source's first 4096-frame chunk raised it). Measured on a
 * private PipeWire: 123 to 153 ms at the default quantum. */
#define STEADY_MAX (160 * MS)

static uint64_t steady(uint64_t t)
{
	uint64_t phase = (t / NS_PER_USEC) % (500 * MS);
	uint64_t tri = phase < 250 * MS ? phase : 500 * MS - phase;
	return 100 * MS + tri * 60 / 250;
}

/* A source that delivers 1 s of audio at once, every second (a high-latency
 * source such as VLC); the monitor raised its target to match. */
static uint64_t bursty(uint64_t t)
{
	uint64_t phase = (t / NS_PER_USEC) % (1000 * MS);
	return 1000 * MS - phase + 5 * MS;
}

static void print_model(const char *name, const struct model *m)
{
	fprintf(stderr, "%s: %u drop(s), %llu ms dropped, %u underrun(s), backlog left %llu ms, max queued %llu ms\n",
		name, m->drops, (unsigned long long)(m->dropped / MS), m->underruns,
		(unsigned long long)(m->backlog / MS), (unsigned long long)(m->max_queued_after / MS));
}

/* Control: normal playback is never cut. */
static void steady_playback_is_not_cut(void **state)
{
	(void)state;
	struct model m = {.base = steady, .target = 85 * MS};
	run(&m, 600 * SECOND_NS);
	print_model("steady, 10 min", &m);
	assert_int_equal(m.drops, 0);
}

/* A 3 s stall: the 3.2 s queued meanwhile must be dropped within a few
 * seconds, back to normal playback, without emptying the queue. */
static void stall_backlog_is_dropped(void **state)
{
	(void)state;
	struct model m = {
		.base = steady,
		.target = 85 * MS,
		.stall_at = 20 * SECOND_NS,
		.stall_backlog = 3200 * MS,
		.check_after = 25 * SECOND_NS,
	};
	run(&m, 60 * SECOND_NS);
	print_model("3.2 s stall", &m);
	assert_int_equal(m.underruns, 0);
	assert_true(m.backlog < 100 * MS);
	assert_true(m.max_queued_after <= STEADY_MAX + 100 * MS);
}

/* A shorter stall leaves a smaller backlog, which must go too. */
static void short_stall_backlog_is_dropped(void **state)
{
	(void)state;
	struct model m = {
		.base = steady,
		.target = 85 * MS,
		.stall_at = 20 * SECOND_NS,
		.stall_backlog = 600 * MS,
		.check_after = 25 * SECOND_NS,
	};
	run(&m, 60 * SECOND_NS);
	print_model("0.6 s stall", &m);
	assert_int_equal(m.underruns, 0);
	assert_true(m.max_queued_after <= STEADY_MAX + 100 * MS);
}

/* Clock drift: the device plays 0.05% slower than OBS produces, so the
 * queue grows by 0.5 ms a second (15 minutes: 450 ms). It must stay
 * bounded. */
static void drift_stays_bounded(void **state)
{
	(void)state;
	struct model m = {
		.base = steady,
		.target = 85 * MS,
		.drift_per_s = 500,
		.check_after = 60 * SECOND_NS,
	};
	run(&m, 900 * SECOND_NS);
	print_model("0.05% drift, 15 min", &m);
	assert_int_equal(m.underruns, 0);
	/* Dropped once the lowest queue in a window passes target + slack. */
	assert_true(m.max_queued_after <= STEADY_MAX + 85 * MS + PULSEAUDIO_MONITOR_BACKLOG_SLACK);
}

/* Control: a source that delivers its audio in bursts empties the queue
 * between bursts and must not be cut. */
static void bursty_source_is_not_cut(void **state)
{
	(void)state;
	struct model m = {.base = bursty, .target = 1000 * MS};
	run(&m, 300 * SECOND_NS);
	print_model("1 s bursts, 5 min", &m);
	assert_int_equal(m.drops, 0);
}

/* Control: a backlog that drains by itself within the window (a burst, not
 * a stall) is not cut. */
static void spike_that_drains_is_not_cut(void **state)
{
	(void)state;
	struct model m = {
		.base = steady,
		.target = 85 * MS,
		.stall_at = 20 * SECOND_NS,
		.stall_backlog = 1000 * MS,
		.drain_ns = 1500000000ULL,
	};
	run(&m, 60 * SECOND_NS);
	print_model("1 s spike draining in 1.5 s", &m);
	assert_int_equal(m.drops, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(steady_playback_is_not_cut),     cmocka_unit_test(stall_backlog_is_dropped),
		cmocka_unit_test(short_stall_backlog_is_dropped), cmocka_unit_test(drift_stays_bounded),
		cmocka_unit_test(bursty_source_is_not_cut),       cmocka_unit_test(spike_that_drains_is_not_cut),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
