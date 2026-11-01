#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "audio-monitoring/osx/coreaudio-pacing.h"

/* Characterization of the CoreAudio monitor's queue pacing, moved from
 * libobs/audio-monitoring/osx/coreaudio-output.c to
 * libobs/audio-monitoring/osx/coreaudio-pacing.c: the buffer and wait sizes,
 * the fill test, the volume gate, the end of the wait, the pause once every
 * buffer is back and what follows a failed enqueue. The file includes no
 * Apple header, so this runs on every platform; the CoreAudio monitor itself
 * only runs on macOS. The functions are hidden in libobs, so this
 * characterizes that C source; the test calls only them. */

/* ------------------------------------------------------------------------ */
/* A stand-in of the monitor, asking the functions as coreaudio-output.c does:
 * fill_buffer(), on_audio_playback() after the resample and
 * coreaudio_buffer_audio(). Buffer ids stand in for AudioQueueBufferRef; the
 * free list holds them in order, as empty_buffers does. */

struct sim {
	size_t buffer_size;
	size_t wait_size;
	size_t queued;
	bool paused;

	int free_ids[COREAUDIO_MONITOR_BUFFERS];
	size_t nfree;
	int queue_ids[COREAUDIO_MONITOR_BUFFERS];
	size_t nqueue;

	/* the next enqueues to fail */
	int fail_enqueues;

	int enqueues;
	int failed;
	int starts;
	int pauses;
	int stops;
};

static void sim_init(struct sim *s, uint32_t channels, uint32_t rate)
{
	*s = (struct sim){0};
	s->buffer_size = coreaudio_monitor_buffer_size(channels, rate);
	s->wait_size = coreaudio_monitor_wait_size(s->buffer_size);
	for (int i = 0; i < COREAUDIO_MONITOR_BUFFERS; i++)
		s->free_ids[s->nfree++] = i;
}

static void sim_push_free(struct sim *s, int id)
{
	assert_true(s->nfree < COREAUDIO_MONITOR_BUFFERS);
	s->free_ids[s->nfree++] = id;
}

static int sim_pop_free(struct sim *s)
{
	int id = s->free_ids[0];

	assert_true(s->nfree > 0);
	for (size_t i = 1; i < s->nfree; i++)
		s->free_ids[i - 1] = s->free_ids[i];
	s->nfree--;
	return id;
}

static size_t sim_free_bytes(const struct sim *s)
{
	return s->nfree * sizeof(void *);
}

/* fill_buffer() */
static bool sim_fill(struct sim *s)
{
	int id;

	if (!coreaudio_monitor_can_fill(s->queued, s->buffer_size))
		return false;

	id = sim_pop_free(s);
	s->queued -= s->buffer_size;
	s->enqueues++;

	if (s->fail_enqueues > 0) {
		s->fail_enqueues--;
		s->failed++;
		s->stops++;

		if (coreaudio_monitor_recover_failed_enqueue()) {
			sim_push_free(s, id);
			s->paused = true;
			s->wait_size = coreaudio_monitor_wait_size(s->buffer_size);
		}
	} else {
		s->queue_ids[s->nqueue++] = id;
	}
	return true;
}

/* on_audio_playback(), after the resample: bytes of audio pushed */
static void sim_play(struct sim *s, size_t bytes)
{
	s->queued += bytes;

	if (coreaudio_monitor_wait_over(s->queued, s->wait_size)) {
		s->wait_size = 0;

		while (s->nfree > 0) {
			if (!sim_fill(s))
				break;
		}

		if (s->paused) {
			s->starts++;
			s->paused = false;
		}
	}
}

/* coreaudio_buffer_audio(): the oldest queued buffer comes back */
static void sim_back(struct sim *s)
{
	int id = s->queue_ids[0];

	assert_true(s->nqueue > 0);
	for (size_t i = 1; i < s->nqueue; i++)
		s->queue_ids[i - 1] = s->queue_ids[i];
	s->nqueue--;

	sim_push_free(s, id);
	while (s->nfree > 0) {
		if (!sim_fill(s))
			break;
	}
	if (coreaudio_monitor_all_back(sim_free_bytes(s), sizeof(void *))) {
		s->paused = true;
		s->wait_size = coreaudio_monitor_wait_size(s->buffer_size);
		s->pauses++;
	}
}

/* ------------------------------------------------------------------------ */

static void the_constants(void **state)
{
	(void)state;

	assert_int_equal(COREAUDIO_MONITOR_BUFFERS, 3);
}

static void the_buffer_is_30_ms_of_float_audio(void **state)
{
	(void)state;

	/* channels * sizeof(float) * rate / 100 * 3 */
	assert_int_equal(coreaudio_monitor_buffer_size(1, 44100), 5292);
	assert_int_equal(coreaudio_monitor_buffer_size(2, 44100), 10584);
	assert_int_equal(coreaudio_monitor_buffer_size(8, 44100), 42336);
	assert_int_equal(coreaudio_monitor_buffer_size(1, 48000), 5760);
	assert_int_equal(coreaudio_monitor_buffer_size(2, 48000), 11520);
	assert_int_equal(coreaudio_monitor_buffer_size(8, 48000), 46080);
	assert_int_equal(coreaudio_monitor_buffer_size(6, 96000), 69120);
	assert_int_equal(coreaudio_monitor_buffer_size(2, 192000), 46080);
}

static void the_buffer_size_divides_after_multiplying(void **state)
{
	(void)state;

	/* The bytes of one second are divided by 100, truncated, then
	 * multiplied by 3: not the frames of 10 ms. */
	assert_int_equal(coreaudio_monitor_buffer_size(1, 44101), 5292);
	assert_int_equal(coreaudio_monitor_buffer_size(1, 22051), 2646);
	assert_int_equal(coreaudio_monitor_buffer_size(3, 22051), 7938);
	assert_int_equal(coreaudio_monitor_buffer_size(1, 25), 3);
	assert_int_equal(coreaudio_monitor_buffer_size(1, 26), 3);
	assert_int_equal(coreaudio_monitor_buffer_size(1, 50), 6);
	assert_int_equal(coreaudio_monitor_buffer_size(3, 25), 9);

	/* Characterized, not endorsed: below 25 Hz a mono buffer has no
	 * bytes, and nothing has none. */
	assert_int_equal(coreaudio_monitor_buffer_size(1, 24), 0);
	assert_int_equal(coreaudio_monitor_buffer_size(1, 0), 0);
	assert_int_equal(coreaudio_monitor_buffer_size(0, 48000), 0);

#if SIZE_MAX == UINT64_MAX
	/* The product is a 64-bit size_t: 8 channels at UINT32_MAX fit, and
	 * UINT32_MAX channels at UINT32_MAX wrap before the division. */
	assert_int_equal(coreaudio_monitor_buffer_size(UINT32_MAX, UINT32_MAX), 0x07ae147aa3d70a3cULL);
	assert_int_equal(coreaudio_monitor_buffer_size(8, UINT32_MAX), 0xf5c28f5aULL);
#endif
}

static void the_wait_is_every_buffer(void **state)
{
	(void)state;

	assert_int_equal(coreaudio_monitor_wait_size(0), 0);
	assert_int_equal(coreaudio_monitor_wait_size(1), 3);
	assert_int_equal(coreaudio_monitor_wait_size(5292), 15876);
	assert_int_equal(coreaudio_monitor_wait_size(11520), 34560);
	/* size_t arithmetic: it wraps */
	assert_int_equal(coreaudio_monitor_wait_size(SIZE_MAX), SIZE_MAX - 2);
}

static void a_buffer_fills_from_a_whole_buffer(void **state)
{
	(void)state;

	assert_false(coreaudio_monitor_can_fill(0, 1));
	assert_false(coreaudio_monitor_can_fill(11519, 11520));
	assert_true(coreaudio_monitor_can_fill(11520, 11520));
	assert_true(coreaudio_monitor_can_fill(11521, 11520));
	assert_true(coreaudio_monitor_can_fill(SIZE_MAX, 11520));
	assert_false(coreaudio_monitor_can_fill(SIZE_MAX - 1, SIZE_MAX));
	assert_true(coreaudio_monitor_can_fill(SIZE_MAX, SIZE_MAX));
	/* Characterized, not endorsed: a zero-byte buffer always fills. */
	assert_true(coreaudio_monitor_can_fill(0, 0));
}

static void the_volume_step_runs_unless_the_volume_is_1(void **state)
{
	(void)state;

	assert_false(coreaudio_monitor_needs_volume(1.0f));
	/* within EPSILON (1e-4f) of 1 */
	assert_false(coreaudio_monitor_needs_volume(1.00009f));
	assert_false(coreaudio_monitor_needs_volume(0.99991f));
	assert_false(coreaudio_monitor_needs_volume(1.0000999f));
	assert_false(coreaudio_monitor_needs_volume(0.9999001f));
	/* just past it once rounded to float */
	assert_true(coreaudio_monitor_needs_volume(1.0001f));
	assert_true(coreaudio_monitor_needs_volume(0.9999f));
	assert_true(coreaudio_monitor_needs_volume(0.0f));
	assert_true(coreaudio_monitor_needs_volume(0.5f));
	assert_true(coreaudio_monitor_needs_volume(2.0f));
	assert_true(coreaudio_monitor_needs_volume(-1.0f));
	assert_true(coreaudio_monitor_needs_volume(INFINITY));
	assert_true(coreaudio_monitor_needs_volume(-INFINITY));
	/* NaN is never close */
	assert_true(coreaudio_monitor_needs_volume(NAN));
}

static void the_wait_ends_at_the_wait_size(void **state)
{
	(void)state;

	assert_false(coreaudio_monitor_wait_over(34559, 34560));
	assert_true(coreaudio_monitor_wait_over(34560, 34560));
	assert_true(coreaudio_monitor_wait_over(34561, 34560));
	/* a cleared wait is over with any data, or none */
	assert_true(coreaudio_monitor_wait_over(0, 0));
	assert_true(coreaudio_monitor_wait_over(1, 0));
	assert_false(coreaudio_monitor_wait_over(SIZE_MAX - 1, SIZE_MAX));
	assert_true(coreaudio_monitor_wait_over(SIZE_MAX, SIZE_MAX));
}

static void the_queue_pauses_at_exactly_three_buffers(void **state)
{
	(void)state;

	/* the free list's size in bytes, three pointers */
	assert_false(coreaudio_monitor_all_back(0, 8));
	assert_false(coreaudio_monitor_all_back(8, 8));
	assert_false(coreaudio_monitor_all_back(16, 8));
	assert_true(coreaudio_monitor_all_back(24, 8));
	assert_false(coreaudio_monitor_all_back(32, 8));
	assert_false(coreaudio_monitor_all_back(23, 8));
	assert_false(coreaudio_monitor_all_back(25, 8));
	/* 32-bit pointers */
	assert_true(coreaudio_monitor_all_back(12, 4));
	assert_false(coreaudio_monitor_all_back(24, 4));
	assert_false(coreaudio_monitor_all_back(8, 4));
	/* this platform's */
	assert_true(coreaudio_monitor_all_back(3 * sizeof(void *), sizeof(void *)));
	/* size_t arithmetic: ptr_size * 3 wraps */
	assert_true(coreaudio_monitor_all_back(SIZE_MAX - 2, SIZE_MAX));
	assert_true(coreaudio_monitor_all_back(0, 0));
}

static void the_monitor_waits_for_three_buffers_then_plays(void **state)
{
	struct sim s;
	(void)state;

	sim_init(&s, 2, 48000);
	assert_int_equal(s.buffer_size, 11520);
	assert_int_equal(s.wait_size, 34560);

	/* below the wait: nothing is filled */
	sim_play(&s, 11520);
	sim_play(&s, 11520);
	assert_int_equal(s.enqueues, 0);
	assert_int_equal(s.wait_size, 34560);

	/* one byte short */
	sim_play(&s, 11519);
	assert_int_equal(s.enqueues, 0);

	/* the wait is over: every buffer fills, the wait is cleared and the
	 * queue, never paused, is not started again */
	sim_play(&s, 1);
	assert_int_equal(s.enqueues, 3);
	assert_int_equal(s.nqueue, 3);
	assert_int_equal(s.nfree, 0);
	assert_int_equal(s.queued, 0);
	assert_int_equal(s.wait_size, 0);
	assert_int_equal(s.starts, 0);
	assert_false(s.paused);

	/* while playing, a returned buffer refills from a whole buffer */
	sim_play(&s, 11519);
	sim_back(&s);
	assert_int_equal(s.enqueues, 3);
	assert_int_equal(s.nfree, 1);
	sim_play(&s, 1);
	assert_int_equal(s.enqueues, 4);
	assert_int_equal(s.nfree, 0);
	sim_play(&s, 11520 * 2 + 7);
	sim_back(&s);
	sim_back(&s);
	assert_int_equal(s.enqueues, 6);
	assert_int_equal(s.queued, 7);
	assert_int_equal(s.pauses, 0);
}

static void the_queue_pauses_when_every_buffer_is_back(void **state)
{
	struct sim s;
	(void)state;

	sim_init(&s, 1, 44100);
	sim_play(&s, s.wait_size);
	assert_int_equal(s.nqueue, 3);

	/* two back with no audio: no pause */
	sim_back(&s);
	sim_back(&s);
	assert_int_equal(s.nfree, 2);
	assert_int_equal(s.pauses, 0);
	assert_false(s.paused);
	assert_int_equal(s.wait_size, 0);

	/* the third: pause, and wait for every buffer's worth again */
	sim_back(&s);
	assert_int_equal(s.nfree, 3);
	assert_int_equal(s.pauses, 1);
	assert_true(s.paused);
	assert_int_equal(s.wait_size, 15876);

	/* resume only once the wait size is queued */
	sim_play(&s, 15875);
	assert_int_equal(s.starts, 0);
	assert_true(s.paused);
	sim_play(&s, 1);
	assert_int_equal(s.starts, 1);
	assert_false(s.paused);
	assert_int_equal(s.wait_size, 0);
	assert_int_equal(s.nqueue, 3);
}

static void the_wait_clears_without_a_pause(void **state)
{
	struct sim s;
	(void)state;

	/* a wait that ends while the queue is not paused clears the wait and
	 * starts nothing */
	sim_init(&s, 1, 48000);
	sim_play(&s, s.wait_size + 5);
	assert_int_equal(s.wait_size, 0);
	assert_int_equal(s.starts, 0);
	assert_int_equal(s.queued, 5);

	/* and a cleared wait is over with every push, even one that fills
	 * nothing */
	sim_back(&s);
	sim_play(&s, 1);
	assert_int_equal(s.wait_size, 0);
	assert_int_equal(s.nfree, 1);
	assert_int_equal(s.starts, 0);
}

static void a_refill_on_return_can_keep_the_queue_from_pausing(void **state)
{
	struct sim s;
	(void)state;

	sim_init(&s, 2, 44100);
	sim_play(&s, s.wait_size + s.buffer_size * 2);
	assert_int_equal(s.queued, s.buffer_size * 2);

	/* each return refills at once while audio lasts */
	for (int i = 0; i < 2; i++) {
		sim_back(&s);
		assert_int_equal(s.nfree, 0);
	}
	assert_int_equal(s.enqueues, 5);
	for (int i = 0; i < 3; i++)
		sim_back(&s);
	assert_int_equal(s.pauses, 1);
	assert_true(s.paused);
}

#ifdef COREAUDIO_PACING_TEST_RUST

/* ------------------------------------------------------------------------ */
/* The Rust port's intentional difference */

static void the_port_recovers_a_failed_enqueue(void **state)
{
	(void)state;

	assert_true(coreaudio_monitor_recover_failed_enqueue());
}

static void the_port_restarts_the_queue_after_a_failed_enqueue(void **state)
{
	struct sim s;
	(void)state;

	sim_init(&s, 2, 48000);
	s.fail_enqueues = 1;
	sim_play(&s, s.wait_size);

	/* the failed buffer is back on the free list, at its end, and the
	 * queue, stopped, is started again with the paused state */
	assert_int_equal(s.failed, 1);
	assert_int_equal(s.stops, 1);
	assert_int_equal(s.enqueues, 3);
	assert_int_equal(s.nqueue, 2);
	assert_int_equal(s.nfree, 1);
	assert_int_equal(s.free_ids[0], 0);
	assert_int_equal(s.starts, 1);
	assert_false(s.paused);
	assert_int_equal(s.wait_size, 34560);

	/* every buffer can come back, so the queue pauses again */
	sim_back(&s);
	sim_back(&s);
	assert_int_equal(s.nfree, 3);
	assert_int_equal(s.pauses, 1);
	assert_true(s.paused);

	/* and the next wait's worth of audio fills all three and starts it */
	sim_play(&s, s.wait_size);
	assert_int_equal(s.nqueue, 3);
	assert_int_equal(s.starts, 2);
	assert_false(s.paused);
}

static void the_port_waits_after_a_failed_refill(void **state)
{
	struct sim s;
	(void)state;

	sim_init(&s, 1, 48000);
	sim_play(&s, s.wait_size + s.buffer_size);

	/* a refill on return fails: the buffer stays free, the monitor
	 * waits, and the next wait's worth starts the queue */
	s.fail_enqueues = 1;
	sim_back(&s);
	assert_int_equal(s.failed, 1);
	assert_int_equal(s.nfree, 1);
	assert_true(s.paused);
	assert_int_equal(s.wait_size, 17280);
	assert_int_equal(s.pauses, 0);

	sim_play(&s, 17279);
	assert_int_equal(s.starts, 0);
	sim_play(&s, 1);
	assert_int_equal(s.starts, 1);
	assert_int_equal(s.nqueue, 3);
	assert_int_equal(s.wait_size, 0);
}

#else

/* ------------------------------------------------------------------------ */
/* Characterized, not endorsed: a failed enqueue stops the queue and keeps
 * the buffer out for good. */

static void the_c_keeps_a_failed_buffer_out(void **state)
{
	(void)state;

	assert_false(coreaudio_monitor_recover_failed_enqueue());
}

static void the_c_never_restarts_after_a_failed_enqueue(void **state)
{
	struct sim s;
	(void)state;

	sim_init(&s, 2, 48000);
	s.fail_enqueues = 1;
	sim_play(&s, s.wait_size);
	assert_int_equal(s.failed, 1);
	assert_int_equal(s.stops, 1);
	assert_int_equal(s.nqueue, 2);
	assert_int_equal(s.nfree, 0);
	assert_int_equal(s.starts, 0);
	assert_false(s.paused);
	assert_int_equal(s.wait_size, 0);

	/* only two buffers can come back: the queue never pauses, so it is
	 * never started again, however much audio follows */
	sim_back(&s);
	sim_back(&s);
	assert_int_equal(s.nfree, 2);
	assert_int_equal(s.pauses, 0);
	assert_false(s.paused);
	for (int i = 0; i < 10; i++)
		sim_play(&s, s.buffer_size * 3);
	assert_int_equal(s.starts, 0);
	assert_int_equal(s.pauses, 0);
}

static void the_c_keeps_filling_after_a_failed_enqueue(void **state)
{
	struct sim s;
	(void)state;

	/* fill_buffer() still returns true: the loop goes on to the next free
	 * buffer */
	sim_init(&s, 1, 48000);
	s.fail_enqueues = 2;
	sim_play(&s, s.wait_size);
	assert_int_equal(s.enqueues, 3);
	assert_int_equal(s.failed, 2);
	assert_int_equal(s.stops, 2);
	assert_int_equal(s.nqueue, 1);
	assert_int_equal(s.nfree, 0);
	assert_int_equal(s.queued, 0);
}

#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(the_constants),
		cmocka_unit_test(the_buffer_is_30_ms_of_float_audio),
		cmocka_unit_test(the_buffer_size_divides_after_multiplying),
		cmocka_unit_test(the_wait_is_every_buffer),
		cmocka_unit_test(a_buffer_fills_from_a_whole_buffer),
		cmocka_unit_test(the_volume_step_runs_unless_the_volume_is_1),
		cmocka_unit_test(the_wait_ends_at_the_wait_size),
		cmocka_unit_test(the_queue_pauses_at_exactly_three_buffers),
		cmocka_unit_test(the_monitor_waits_for_three_buffers_then_plays),
		cmocka_unit_test(the_queue_pauses_when_every_buffer_is_back),
		cmocka_unit_test(the_wait_clears_without_a_pause),
		cmocka_unit_test(a_refill_on_return_can_keep_the_queue_from_pausing),
#ifdef COREAUDIO_PACING_TEST_RUST
		cmocka_unit_test(the_port_recovers_a_failed_enqueue),
		cmocka_unit_test(the_port_restarts_the_queue_after_a_failed_enqueue),
		cmocka_unit_test(the_port_waits_after_a_failed_refill),
#else
		cmocka_unit_test(the_c_keeps_a_failed_buffer_out),
		cmocka_unit_test(the_c_never_restarts_after_a_failed_enqueue),
		cmocka_unit_test(the_c_keeps_filling_after_a_failed_enqueue),
#endif
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
