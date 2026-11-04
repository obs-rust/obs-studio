/*
 * Characterization before source_output_audio_data() of obs-source.c moves to
 * its own file and is ported (#3628), with handle_ts_jump(), the place and the
 * push back of a packet, and the reset functions.
 *
 * test_source_audio_timing_steps and test_source_audio_place_steps pin the
 * decisions on plain values. test_audio_clock_drift and
 * test_audio_sync_offset_change pin the audio in use. This test pins what the
 * path around the decisions does with the fields of the source and its buffer,
 * for each kind of packet: which fields it stores, where the packet goes, and
 * what the capture callback sees.
 *
 * These are characterized, not endorsed. One example: the first placed packet
 * leaves next_audio_sys_ts_min at the packet's own start, not at its end,
 * because reset_audio_data() runs after the store.
 *
 * The packets go in through obs_source_output_audio(), on one thread. The
 * audio thread waits in a task for each case, so it does not consume the
 * buffer. The source's format is the output format, so no resampler runs.
 * obs_startup() needs a display on Linux, so the test skips without one.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs-internal.h>
#include <util/deque.h>
#include <util/platform.h>
#include <util/util_uint64.h>

#define SOURCE_ID "test_source_audio_output_source"

#define RATE 48000
#define FRAMES 1024
#define CHANNELS 2
#define SAMPLE_BYTES sizeof(float)
#define SEC 1000000000ULL
#define MSEC 1000000ULL
#define MAX_PACKETS 1000 /* MAX_BUF_SIZE in obs-internal.h, in FRAMES */

/* far from os_gettime_ns(), as a media file's timeline is */
#define FAR (3600 * SEC)

static bool obs_up;

static uint64_t duration(void)
{
	return util_mul_div64(FRAMES, SEC, RATE);
}

/* get_buf_placement() in frames */
static size_t frames_after(uint64_t timestamp, uint64_t audio_ts)
{
	return (size_t)util_mul_div64(timestamp - audio_ts, RATE, SEC);
}

static const char *source_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "audio output";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void source_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}
	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}
	struct obs_source_info info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
	};
	obs_register_source(&info);
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

/* ------------------------------------------------------------------------ */

struct fixture {
	obs_source_t *source;
	os_event_t *parked;
	os_event_t *resume;
};

static void park_task(void *param)
{
	struct fixture *f = param;

	os_event_signal(f->parked);
	os_event_wait(f->resume);
}

static void done_task(void *param)
{
	UNUSED_PARAMETER(param);
}

static int case_setup(void **state)
{
	if (!obs_up) {
		skip();
		return 0;
	}
	struct fixture *f = calloc(1, sizeof(*f));
	assert_non_null(f);
	f->source = obs_source_create_private(SOURCE_ID, "audio output", NULL);
	assert_non_null(f->source);
	assert_int_equal(os_event_init(&f->parked, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&f->resume, OS_EVENT_TYPE_AUTO), 0);
	obs_queue_task(OBS_TASK_AUDIO, park_task, f, false);
	os_event_wait(f->parked);
	*state = f;
	return 0;
}

static int case_teardown(void **state)
{
	struct fixture *f = *state;

	if (!f)
		return 0;
	os_event_signal(f->resume);
	/* Tasks run in order: this one returns after the park task has ended. */
	obs_queue_task(OBS_TASK_AUDIO, done_task, NULL, true);
	os_event_destroy(f->parked);
	os_event_destroy(f->resume);
	obs_source_remove(f->source);
	obs_source_release(f->source);
	free(f);
	return 0;
}

/* The left channel has `value`, the right one its negative. */
static void send(obs_source_t *source, uint64_t timestamp, float value)
{
	static float left[FRAMES];
	static float right[FRAMES];

	for (size_t i = 0; i < FRAMES; i++) {
		left[i] = value;
		right[i] = -value;
	}
	struct obs_source_audio audio = {
		.data = {(const uint8_t *)left, (const uint8_t *)right},
		.frames = FRAMES,
		.speakers = SPEAKERS_STEREO,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.samples_per_sec = RATE,
		.timestamp = timestamp,
	};
	obs_source_output_audio(source, &audio);
}

static size_t buffered_frames(const obs_source_t *source)
{
	return source->audio_input_buf[0].size / SAMPLE_BYTES;
}

/* Both channels of [first, first + count) in the buffer hold value. */
static void assert_buffered(obs_source_t *source, size_t first, size_t count, float value)
{
	size_t frames = buffered_frames(source);
	float *data = malloc((frames + 1) * SAMPLE_BYTES);

	assert_non_null(data);
	assert_true(first + count <= frames);
	for (size_t ch = 0; ch < CHANNELS; ch++) {
		assert_int_equal(source->audio_input_buf[ch].size, frames * SAMPLE_BYTES);
		deque_peek_front(&source->audio_input_buf[ch], data, frames * SAMPLE_BYTES);
		for (size_t i = first; i < first + count; i++) {
			const float expected = ch ? -value : value;
			if (data[i] != expected) {
				fprintf(stderr, "channel %zu frame %zu: %f, expected %f\n", ch, i, (double)data[i],
					(double)expected);
				fail();
			}
		}
	}
	free(data);
}

/* A value read from os_gettime_ns() inside the call. */
static void assert_between(uint64_t value, uint64_t low, uint64_t high)
{
	assert_true(value >= low);
	assert_true(value <= high);
}

/* ------------------------------------------------------------------------ */

/* The first packet, stamped with the system clock: no adjustment, placed at
 * its stamp. reset_audio_data() runs after next_audio_sys_ts_min is stored, so
 * it holds the packet's start. */
static void a_packet_on_the_system_clock_is_placed_at_its_stamp(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	s->last_audio_input_buf_size = 7;
	send(s, t, 0.25f);

	assert_true(s->timing_set);
	assert_int_equal(s->timing_adjust, 0);
	assert_int_equal(s->audio_ts, t);
	assert_int_equal(s->next_audio_ts_min, t + duration());
	assert_int_equal(s->next_audio_sys_ts_min, t);
	assert_int_equal(s->last_audio_input_buf_size, 0);
	assert_int_equal(buffered_frames(s), FRAMES);
	assert_buffered(s, 0, FRAMES, 0.25f);
}

/* A packet within the smoothing threshold continues the last one: it is
 * appended, audio_ts stays, and the next expected stamp counts samples, not
 * the stamps. */
static void a_continuing_packet_is_appended(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	send(s, t, 0.25f);
	s->last_audio_input_buf_size = 7;
	send(s, t + duration() + MSEC, 0.5f);

	assert_int_equal(s->audio_ts, t);
	assert_int_equal(s->next_audio_ts_min, t + 2 * duration());
	assert_int_equal(s->next_audio_sys_ts_min, t + 2 * duration());
	assert_int_equal(s->last_audio_input_buf_size, 0);
	assert_int_equal(buffered_frames(s), 2 * FRAMES);
	assert_buffered(s, 0, FRAMES, 0.25f);
	assert_buffered(s, FRAMES, FRAMES, 0.5f);

	/* exactly the expected system time */
	send(s, t + 2 * duration(), 0.75f);
	assert_int_equal(s->audio_ts, t);
	assert_int_equal(buffered_frames(s), 3 * FRAMES);
	assert_buffered(s, 2 * FRAMES, FRAMES, 0.75f);
}

/* A stamp far from the system clock, with no timing yet: timing_adjust maps it
 * to the system clock, and the packet is placed there. */
static void a_media_stamp_sets_the_timing(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t before = os_gettime_ns();
	const uint64_t ts = before + FAR;

	send(s, ts, 0.25f);
	const uint64_t after = os_gettime_ns();

	assert_true(s->timing_set);
	assert_between(ts + s->timing_adjust, before, after);
	assert_int_equal(s->audio_ts, ts + s->timing_adjust);
	assert_int_equal(s->next_audio_ts_min, ts + duration());
	assert_int_equal(s->next_audio_sys_ts_min, s->audio_ts);
	assert_int_equal(buffered_frames(s), FRAMES);
}

/* A media stamp that jumps by more than MAX_TS_VAR: handle_ts_jump() empties
 * the buffer and resets the timing, and the packet is appended to the emptied
 * buffer. */
static void a_timestamp_jump_empties_the_buffer(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t ts = os_gettime_ns() + FAR;

	send(s, ts, 0.25f);
	send(s, ts + duration(), 0.25f);
	assert_int_equal(buffered_frames(s), 2 * FRAMES);

	const uint64_t ts2 = ts + 10 * SEC;
	const uint64_t before = os_gettime_ns();
	s->last_audio_input_buf_size = 7;
	send(s, ts2, 0.5f);
	const uint64_t after = os_gettime_ns();

	assert_true(s->timing_set);
	assert_between(ts2 + s->timing_adjust, before, after);
	assert_int_equal(s->audio_ts, ts2 + s->timing_adjust);
	assert_int_equal(s->next_audio_ts_min, ts2 + duration());
	assert_int_equal(s->next_audio_sys_ts_min, ts2 + duration() + s->timing_adjust);
	assert_int_equal(s->last_audio_input_buf_size, 0);
	assert_int_equal(buffered_frames(s), FRAMES);
	assert_buffered(s, 0, FRAMES, 0.5f);
}

/* After a jump the emptied buffer starts at the system time shifted by the
 * sync offset, as a packet placed on its own would (#286). */
static void a_timestamp_jump_keeps_the_sync_offset(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const int64_t offset = 50 * (int64_t)MSEC;
	const uint64_t ts = os_gettime_ns() + FAR;

	obs_source_set_sync_offset(s, offset);
	send(s, ts, 0.25f);
	assert_int_equal(s->last_sync_offset, offset);
	assert_int_equal(s->audio_ts, ts + s->timing_adjust + (uint64_t)offset);

	const uint64_t ts2 = ts + 10 * SEC;
	send(s, ts2, 0.5f);

	assert_int_equal(s->audio_ts, ts2 + s->timing_adjust + (uint64_t)offset);
	assert_int_equal(buffered_frames(s), FRAMES);
	assert_buffered(s, 0, FRAMES, 0.5f);
}

/* Past the smoothing threshold but within MAX_TS_VAR: no reset, the stamp
 * stands, and the packet is placed by it. The gap holds silence. */
static void a_later_packet_is_placed_after_a_gap(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();
	const uint64_t t2 = t + duration() + 100 * MSEC;
	const size_t place = frames_after(t2, t);

	send(s, t, 0.25f);
	s->last_audio_input_buf_size = 7;
	send(s, t2, 0.5f);

	assert_int_equal(s->audio_ts, t);
	assert_int_equal(s->next_audio_ts_min, t2 + duration());
	assert_int_equal(s->next_audio_sys_ts_min, t2 + duration());
	assert_int_equal(s->last_audio_input_buf_size, 0);
	assert_int_equal(buffered_frames(s), place + FRAMES);
	assert_buffered(s, 0, FRAMES, 0.25f);
	assert_buffered(s, FRAMES, place - FRAMES, 0.0f);
	assert_buffered(s, place, FRAMES, 0.5f);
}

/* A packet older than audio_ts: the buffer starts again at that packet. */
static void an_older_packet_resets_the_buffer(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();
	const uint64_t older = t - 100 * MSEC;

	send(s, t, 0.25f);
	send(s, older, 0.5f);

	assert_int_equal(s->audio_ts, older);
	assert_int_equal(s->next_audio_ts_min, older + duration());
	assert_int_equal(s->next_audio_sys_ts_min, older);
	assert_int_equal(buffered_frames(s), FRAMES);
	assert_buffered(s, 0, FRAMES, 0.5f);
}

/* A changed sync offset places the next packet by its stamp, even if it
 * continues the last one (#333). */
static void a_changed_sync_offset_places_the_next_packet(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const int64_t offset = 30 * (int64_t)MSEC;
	const uint64_t t = os_gettime_ns();

	send(s, t, 0.25f);
	send(s, t + duration(), 0.25f);
	assert_int_equal(s->last_sync_offset, 0);

	obs_source_set_sync_offset(s, offset);
	const uint64_t t3 = t + 2 * duration();
	const size_t place = frames_after(t3 + (uint64_t)offset, t);
	send(s, t3, 0.5f);

	assert_int_equal(s->last_sync_offset, offset);
	assert_int_equal(s->audio_ts, t);
	assert_int_equal(buffered_frames(s), place + FRAMES);
	assert_buffered(s, 2 * FRAMES, place - 2 * FRAMES, 0.0f);
	assert_buffered(s, place, FRAMES, 0.5f);
}

/* A source that only monitors keeps its timing and buffers nothing. */
static void a_monitor_only_source_buffers_nothing(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	s->monitoring_type = OBS_MONITORING_TYPE_MONITOR_ONLY;
	send(s, t, 0.25f);

	assert_true(s->timing_set);
	assert_int_equal(s->audio_ts, 0);
	assert_int_equal(s->next_audio_ts_min, t + duration());
	assert_int_equal(s->next_audio_sys_ts_min, t + duration());
	assert_int_equal(buffered_frames(s), 0);
}

/* The buffer stops at MAX_BUF_SIZE: an appended packet that does not fit is
 * dropped, and the timing goes on. */
static void the_buffer_stops_at_its_maximum(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();
	const uint64_t step = duration();

	for (size_t i = 0; i < MAX_PACKETS; i++)
		send(s, t + i * step, 0.25f);
	assert_int_equal(buffered_frames(s), MAX_PACKETS * FRAMES);

	s->last_audio_input_buf_size = 7;
	send(s, t + MAX_PACKETS * step, 0.5f);

	assert_int_equal(buffered_frames(s), MAX_PACKETS * FRAMES);
	assert_int_equal(s->last_audio_input_buf_size, 7);
	assert_int_equal(s->next_audio_ts_min, t + (MAX_PACKETS + 1) * step);
}

/* The source's system time moved by more than MAX_TS_VAR and the stamp did not
 * continue: the timing is reset to the packet (#522). */
static void a_moved_system_time_resets_a_discontinuous_packet(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t ts = os_gettime_ns() + FAR;

	send(s, ts, 0.25f);
	s->timing_adjust = s->timing_adjust + 3 * SEC;

	const uint64_t ts2 = ts + duration() + 100 * MSEC;
	const uint64_t before = os_gettime_ns();
	send(s, ts2, 0.5f);
	const uint64_t after = os_gettime_ns();

	assert_between(ts2 + s->timing_adjust, before, after);
	assert_int_equal(s->next_audio_ts_min, ts2 + duration());
	assert_int_equal(s->next_audio_sys_ts_min, ts2 + duration() + s->timing_adjust);
}

/* If the stamp did continue, the audio follows the moved timing (a Video Delay
 * (Async) filter), and the packet is placed there. */
static void a_moved_system_time_keeps_a_continuous_packet(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t ts = os_gettime_ns() + FAR;

	send(s, ts, 0.25f);
	const uint64_t audio_ts = s->audio_ts;
	const uint64_t adjust = s->timing_adjust + 3 * SEC;
	s->timing_adjust = adjust;

	const uint64_t ts2 = ts + duration();
	const size_t place = frames_after(ts2 + adjust, audio_ts);
	send(s, ts2, 0.5f);

	assert_int_equal(s->timing_adjust, adjust);
	assert_int_equal(s->audio_ts, audio_ts);
	assert_int_equal(s->next_audio_sys_ts_min, ts2 + duration() + adjust);
	assert_int_equal(buffered_frames(s), place + FRAMES);
	assert_buffered(s, place, FRAMES, 0.5f);
}

/* An unbuffered, decoupled source re-anchors on each packet that continues. */
static void a_decoupled_unbuffered_source_reanchors(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t ts = os_gettime_ns() + FAR;

	obs_source_set_async_unbuffered(s, true);
	obs_source_set_async_decoupled(s, true);
	send(s, ts, 0.25f);

	const uint64_t ts2 = ts + duration() + MSEC;
	const uint64_t before = os_gettime_ns();
	send(s, ts2, 0.5f);
	const uint64_t after = os_gettime_ns();

	assert_between(ts2 + s->timing_adjust, before, after);
	assert_int_equal(s->next_audio_ts_min, ts + 2 * duration());
}

/* obs_source_set_async_decoupled() clears the timing and the buffer. */
static void decoupling_clears_the_timing_and_the_buffer(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	send(s, t, 0.25f);
	s->last_audio_input_buf_size = 7;
	const uint64_t next_min = s->next_audio_ts_min;

	obs_source_set_async_decoupled(s, true);

	assert_false(s->timing_set);
	assert_int_equal(s->audio_ts, 0);
	assert_int_equal(s->next_audio_sys_ts_min, 0);
	assert_int_equal(s->last_audio_input_buf_size, 0);
	assert_int_equal(s->next_audio_ts_min, next_min);
	assert_int_equal(buffered_frames(s), 0);

	/* clearing again with the flag off does nothing */
	send(s, t + duration(), 0.25f);
	obs_source_set_async_decoupled(s, false);
	assert_int_equal(buffered_frames(s), FRAMES);
}

struct capture {
	obs_source_t *source;
	int count;
	uint64_t timestamp;
	uint32_t frames;
	bool muted;
	float first_left;
	size_t buffered;
	bool buf_mutex_free;
};

static void capture_callback(void *param, obs_source_t *source, const struct audio_data *data, bool muted)
{
	struct capture *c = param;

	assert_ptr_equal(source, c->source);
	c->count++;
	c->timestamp = data->timestamp;
	c->frames = data->frames;
	c->muted = muted;
	c->first_left = ((const float *)data->data[0])[0];
	c->buffered = buffered_frames(source);
	c->buf_mutex_free = pthread_mutex_trylock(&source->audio_buf_mutex) == 0;
	if (c->buf_mutex_free)
		pthread_mutex_unlock(&source->audio_buf_mutex);
}

/* The capture callback runs after the packet is in the buffer, with
 * audio_buf_mutex free, and gets the packet's own stamp, not the smoothed one. */
static void the_capture_callback_gets_the_packet_as_it_came(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	struct capture c = {.source = s};
	const uint64_t t = os_gettime_ns();
	const uint64_t t2 = t + duration() + MSEC;

	obs_source_add_audio_capture_callback(s, capture_callback, &c);

	send(s, t, 0.25f);
	assert_int_equal(c.count, 1);
	assert_int_equal(c.timestamp, t);
	assert_int_equal(c.frames, FRAMES);
	assert_false(c.muted);
	assert_true(c.first_left == 0.25f);
	assert_int_equal(c.buffered, FRAMES);
	assert_true(c.buf_mutex_free);

	send(s, t2, 0.5f);
	assert_int_equal(c.count, 2);
	assert_int_equal(c.timestamp, t2);
	assert_int_equal(c.buffered, 2 * FRAMES);

	obs_source_set_muted(s, true);
	send(s, t + 2 * duration(), 0.5f);
	assert_int_equal(c.count, 3);
	assert_true(c.muted);
	/* muted audio is still buffered */
	assert_int_equal(c.buffered, 3 * FRAMES);

	obs_source_remove_audio_capture_callback(s, capture_callback, &c);
}

/* A monitor-only source still calls the capture callback, and the disabled
 * flag reaches it as muted. */
static void the_capture_callback_runs_for_a_monitor_only_source(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	struct capture c = {.source = s};

	s->monitoring_type = OBS_MONITORING_TYPE_MONITOR_ONLY;
	obs_source_set_enabled(s, false);
	obs_source_add_audio_capture_callback(s, capture_callback, &c);

	send(s, os_gettime_ns(), 0.25f);

	assert_int_equal(c.count, 1);
	assert_true(c.muted);
	assert_int_equal(c.buffered, 0);

	obs_source_remove_audio_capture_callback(s, capture_callback, &c);
}

int main(void)
{
#define CASE(name) cmocka_unit_test_setup_teardown(name, case_setup, case_teardown)
	const struct CMUnitTest tests[] = {
		CASE(a_packet_on_the_system_clock_is_placed_at_its_stamp),
		CASE(a_continuing_packet_is_appended),
		CASE(a_media_stamp_sets_the_timing),
		CASE(a_timestamp_jump_empties_the_buffer),
		CASE(a_timestamp_jump_keeps_the_sync_offset),
		CASE(a_later_packet_is_placed_after_a_gap),
		CASE(an_older_packet_resets_the_buffer),
		CASE(a_changed_sync_offset_places_the_next_packet),
		CASE(a_monitor_only_source_buffers_nothing),
		CASE(the_buffer_stops_at_its_maximum),
		CASE(a_moved_system_time_resets_a_discontinuous_packet),
		CASE(a_moved_system_time_keeps_a_continuous_packet),
		CASE(a_decoupled_unbuffered_source_reanchors),
		CASE(decoupling_clears_the_timing_and_the_buffer),
		CASE(the_capture_callback_gets_the_packet_as_it_came),
		CASE(the_capture_callback_runs_for_a_monitor_only_source),
	};
#undef CASE
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
