/*
 * Characterization before obs_source_output_audio() of obs-source.c moves to its
 * own file and is ported (#3632), with process_audio(), reset_resampler() and
 * copy_audio_data().
 *
 * test_source_audio_input_steps pins the decisions on plain values, and
 * test_source_audio_balance_mono and test_audio_restart_resampler_tail pin the
 * audio in use. This test pins what the path around the decisions does with the
 * fields of the source: when the resampler changes, what happens when it cannot
 * be created, what the filters get and what they may do, and the checks at the
 * start.
 *
 * These are characterized, not endorsed. One example: when the resampler cannot
 * be created, process_audio() returns, and obs_source_output_audio() still sends
 * the previous packet's samples on, with the previous packet's stamp.
 *
 * The audio thread waits in a task for each case, so it does not consume the
 * buffer. obs_startup() needs a display on Linux, so the test skips without one.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs-internal.h>
#include <util/platform.h>
#include <util/util_uint64.h>

#define SOURCE_ID "test_source_audio_input_source"
#define FILTER_ID "test_source_audio_input_filter"

#define RATE 48000
#define OTHER_RATE 44100
#define FRAMES 1024
#define SEC 1000000000ULL
#define MSEC 1000000ULL

static bool obs_up;

static const char *source_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "audio input";
}

static void source_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

/* What the filter callback does, per filter. */
struct filter_rule {
	const char *name;
	bool drop;
	struct obs_audio_data *replace;
	int count;
	struct obs_audio_data *seen;
	uint64_t seen_timestamp;
	uint32_t seen_frames;
};

static struct filter_rule *rules[4];
static size_t next_rule;
static char order[16];
static size_t order_len;

static void *filter_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	UNUSED_PARAMETER(source);
	return rules[next_rule++];
}

static struct obs_audio_data *filter_audio(void *data, struct obs_audio_data *audio)
{
	struct filter_rule *rule = data;

	rule->count++;
	rule->seen = audio;
	rule->seen_timestamp = audio->timestamp;
	rule->seen_frames = audio->frames;
	order[order_len++] = rule->name[0];
	if (rule->drop)
		return NULL;
	return rule->replace ? rule->replace : audio;
}

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	UNUSED_PARAMETER(level);
	UNUSED_PARAMETER(format);
	UNUSED_PARAMETER(args);
	UNUSED_PARAMETER(param);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	base_set_log_handler(quiet_log, NULL);
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
	struct obs_source_info filter = {
		.id = FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = source_name,
		.create = filter_create,
		.destroy = source_destroy,
		.filter_audio = filter_audio,
	};
	obs_register_source(&filter);
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
	f->source = obs_source_create_private(SOURCE_ID, "audio input", NULL);
	assert_non_null(f->source);
	assert_int_equal(os_event_init(&f->parked, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&f->resume, OS_EVENT_TYPE_AUTO), 0);
	obs_queue_task(OBS_TASK_AUDIO, park_task, f, false);
	os_event_wait(f->parked);
	next_rule = 0;
	order_len = 0;
	memset(order, 0, sizeof(order));
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

static float left[8192];
static float right[8192];

static struct obs_source_audio packet(uint64_t timestamp, uint32_t frames, uint32_t rate, float value)
{
	for (uint32_t i = 0; i < frames; i++) {
		left[i] = value;
		right[i] = -value;
	}
	struct obs_source_audio audio = {
		.data = {(const uint8_t *)left, (const uint8_t *)right},
		.frames = frames,
		.speakers = SPEAKERS_STEREO,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.samples_per_sec = rate,
		.timestamp = timestamp,
	};
	return audio;
}

static void send(obs_source_t *s, uint64_t timestamp, uint32_t frames, uint32_t rate, float value)
{
	struct obs_source_audio audio = packet(timestamp, frames, rate, value);
	obs_source_output_audio(s, &audio);
}

static obs_source_t *add_filter(obs_source_t *s, struct filter_rule *rule)
{
	rules[next_rule] = rule;
	obs_source_t *filter = obs_source_create_private(FILTER_ID, rule->name, NULL);
	assert_non_null(filter);
	obs_source_filter_add(s, filter);
	return filter;
}

struct capture {
	int count;
	uint64_t timestamp;
	uint32_t frames;
	float first_left;
};

static void capture_callback(void *param, obs_source_t *source, const struct audio_data *data, bool muted)
{
	struct capture *c = param;

	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(muted);
	c->count++;
	c->timestamp = data->timestamp;
	c->frames = data->frames;
	c->first_left = ((const float *)data->data[0])[0];
}

/* ------------------------------------------------------------------------ */

/* The output's format needs no resampler: the samples are copied as they came,
 * and the stored format and the end of the packet are kept. */
static void a_packet_in_the_output_format_is_copied(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	send(s, t, FRAMES, RATE, 0.25f);

	assert_null(s->resampler);
	assert_false(s->audio_failed);
	assert_int_equal(s->sample_info.samples_per_sec, RATE);
	assert_int_equal(s->sample_info.format, AUDIO_FORMAT_FLOAT_PLANAR);
	assert_int_equal(s->sample_info.speakers, SPEAKERS_STEREO);
	assert_int_equal(s->next_resample_in_ts, t + util_mul_div64(FRAMES, SEC, RATE));
	assert_int_equal(s->audio_data.frames, FRAMES);
	assert_int_equal(s->audio_data.timestamp, t);
	assert_int_equal(s->audio_storage_size, FRAMES * sizeof(float));
	assert_true(((const float *)s->audio_data.data[0])[FRAMES - 1] == 0.25f);
	assert_true(((const float *)s->audio_data.data[1])[0] == -0.25f);
	assert_int_equal(s->resample_offset, 0);
}

/* The storage grows with a larger packet and stays with a smaller one. */
static void the_storage_only_grows(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	send(s, t, FRAMES, RATE, 0.25f);
	send(s, t + util_mul_div64(FRAMES, SEC, RATE), 2 * FRAMES, RATE, 0.5f);
	assert_int_equal(s->audio_storage_size, 2 * FRAMES * sizeof(float));
	const uint8_t *grown = s->audio_data.data[0];

	send(s, t + util_mul_div64(3 * FRAMES, SEC, RATE), FRAMES, RATE, 0.75f);
	assert_int_equal(s->audio_storage_size, 2 * FRAMES * sizeof(float));
	assert_ptr_equal(s->audio_data.data[0], grown);
	assert_int_equal(s->audio_data.frames, FRAMES);
}

/* A different format creates a resampler, and the output's format removes it. */
static void a_format_change_creates_and_removes_the_resampler(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	send(s, t, FRAMES, OTHER_RATE, 0.25f);
	assert_non_null(s->resampler);
	assert_false(s->audio_failed);
	assert_int_equal(s->sample_info.samples_per_sec, OTHER_RATE);
	assert_int_equal(s->next_resample_in_ts, t + util_mul_div64(FRAMES, SEC, OTHER_RATE));
	send(s, s->next_resample_in_ts, FRAMES, OTHER_RATE, 0.25f);
	/* the resampler holds some back for its filter */
	assert_true(s->resample_offset > 0);
	assert_true(s->audio_data.frames > 0);

	send(s, t + SEC, FRAMES, RATE, 0.5f);
	assert_null(s->resampler);
	assert_int_equal(s->sample_info.samples_per_sec, RATE);
	assert_int_equal(s->resample_offset, 0);
	assert_int_equal(s->audio_data.frames, FRAMES);
}

/* A packet that does not continue the last one restarts the resampler (#546):
 * its offset starts again. */
static void a_discontinuity_restarts_the_resampler(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	send(s, t, FRAMES, OTHER_RATE, 0.25f);
	const uint64_t end = s->next_resample_in_ts;
	send(s, end, FRAMES, OTHER_RATE, 0.25f);
	const uint32_t continued = s->audio_data.frames;
	const uint64_t held = s->resample_offset;

	/* just under the threshold: the same stream */
	const uint64_t next = s->next_resample_in_ts + 69 * MSEC;
	send(s, next, FRAMES, OTHER_RATE, 0.25f);
	assert_true(s->resample_offset >= held / 2);
	assert_true(s->audio_data.frames >= continued - 1);

	/* at the threshold: a new stream, whose first packet comes out short */
	send(s, s->next_resample_in_ts + 70 * MSEC, FRAMES, OTHER_RATE, 0.25f);
	assert_non_null(s->resampler);
	assert_true(s->audio_data.frames < continued);
}

/* The compensation flag needs a resampler for the output's format too. */
static void the_compensation_flag_creates_and_removes_the_resampler(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	const uint64_t t = os_gettime_ns();

	send(s, t, FRAMES, RATE, 0.25f);
	assert_null(s->resampler);
	assert_false(s->last_async_compensation);

	obs_source_set_async_compensation(s, true);
	send(s, s->next_resample_in_ts, FRAMES, RATE, 0.25f);
	assert_non_null(s->resampler);
	assert_true(s->last_async_compensation);

	obs_source_set_async_compensation(s, false);
	send(s, s->next_resample_in_ts, FRAMES, RATE, 0.25f);
	assert_null(s->resampler);
	assert_false(s->last_async_compensation);
}

/* A resampler that cannot be created: audio_failed is set, nothing is copied, and
 * the previous packet goes on again. The next good format clears it. */
static void a_failed_resampler_sends_the_previous_packet_again(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	struct capture c = {0};
	const uint64_t t = os_gettime_ns();

	obs_source_add_audio_capture_callback(s, capture_callback, &c);
	send(s, t, FRAMES, RATE, 0.25f);
	assert_int_equal(c.count, 1);
	const uint64_t end = s->next_resample_in_ts;

	/* no sample rate: libswresample refuses it, and the end is not stored */
	send(s, t + SEC, FRAMES, 0, 0.5f);
	assert_true(s->audio_failed);
	assert_null(s->resampler);
	assert_int_equal(s->next_resample_in_ts, end);
	assert_int_equal(s->sample_info.samples_per_sec, 0);
	assert_int_equal(c.count, 2);
	assert_int_equal(c.timestamp, t);
	assert_true(c.first_left == 0.25f);

	send(s, t + 2 * SEC, FRAMES, RATE, 0.75f);
	assert_false(s->audio_failed);
	assert_int_equal(c.count, 3);
	assert_true(c.first_left == 0.75f);

	obs_source_remove_audio_capture_callback(s, capture_callback, &c);
}

/* A plane that the format does not have is cleared before the resampler reads
 * the packet: the pointer here would crash. */
static void unused_planes_are_cleared(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	struct obs_source_audio audio = packet(os_gettime_ns(), FRAMES, OTHER_RATE, 0.25f);

	audio.speakers = SPEAKERS_MONO;
	audio.data[1] = (const uint8_t *)(uintptr_t)1;
	audio.data[7] = (const uint8_t *)(uintptr_t)1;
	obs_source_output_audio(s, &audio);

	assert_non_null(s->resampler);
	assert_int_equal(s->sample_info.speakers, SPEAKERS_MONO);
	/* the caller's packet is left alone */
	assert_ptr_equal(audio.data[1], (const uint8_t *)(uintptr_t)1);
}

/* The filters run last to first on the source's own audio_data, and a disabled
 * filter is passed over. */
static void the_filters_run_last_to_first(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	struct filter_rule a = {.name = "a"};
	struct filter_rule b = {.name = "b"};
	struct filter_rule c = {.name = "c"};
	struct capture cap = {0};
	const uint64_t t = os_gettime_ns();

	obs_source_t *fa = add_filter(s, &a);
	obs_source_t *fb = add_filter(s, &b);
	obs_source_t *fc = add_filter(s, &c);
	obs_source_set_enabled(fb, false);
	obs_source_add_audio_capture_callback(s, capture_callback, &cap);

	send(s, t, FRAMES, RATE, 0.25f);

	/* obs_source_filter_add() puts a new filter first in the list, so the
	 * one added first runs first */
	assert_string_equal(order, "ac");
	assert_int_equal(b.count, 0);
	assert_ptr_equal(a.seen, &s->audio_data);
	assert_int_equal(a.seen_timestamp, t);
	assert_int_equal(a.seen_frames, FRAMES);
	assert_int_equal(cap.count, 1);
	assert_int_equal(cap.timestamp, t);

	obs_source_remove_audio_capture_callback(s, capture_callback, &cap);
	obs_source_filter_remove(s, fa);
	obs_source_filter_remove(s, fb);
	obs_source_filter_remove(s, fc);
	obs_source_release(fa);
	obs_source_release(fb);
	obs_source_release(fc);
}

/* A filter that returns NULL drops the packet: no later filter, no buffer, no
 * capture callback. */
static void a_filter_can_drop_the_packet(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	struct filter_rule keep = {.name = "k"};
	struct filter_rule drop = {.name = "d", .drop = true};
	struct capture cap = {0};

	/* added first, so it runs first */
	obs_source_t *fd = add_filter(s, &drop);
	obs_source_t *fk = add_filter(s, &keep);
	obs_source_add_audio_capture_callback(s, capture_callback, &cap);

	send(s, os_gettime_ns(), FRAMES, RATE, 0.25f);

	assert_string_equal(order, "d");
	assert_int_equal(keep.count, 0);
	assert_int_equal(cap.count, 0);
	assert_int_equal(s->audio_input_buf[0].size, 0);
	assert_int_equal(s->next_audio_ts_min, 0);
	/* process_audio() ran */
	assert_int_equal(s->audio_data.frames, FRAMES);

	obs_source_remove_audio_capture_callback(s, capture_callback, &cap);
	obs_source_filter_remove(s, fk);
	obs_source_filter_remove(s, fd);
	obs_source_release(fk);
	obs_source_release(fd);
}

/* A filter can return another packet, and its stamp and frames go on from there.
 * The source's audio_data stays as it was. */
static void a_filter_can_replace_the_packet(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	static float replaced_left[64];
	static float replaced_right[64];
	const uint64_t t = os_gettime_ns();
	struct obs_audio_data other = {
		.data = {(uint8_t *)replaced_left, (uint8_t *)replaced_right},
		.frames = 64,
		.timestamp = t + MSEC,
	};
	struct filter_rule rule = {.name = "r", .replace = &other};
	struct capture cap = {0};

	replaced_left[0] = 0.75f;

	obs_source_t *fr = add_filter(s, &rule);
	obs_source_add_audio_capture_callback(s, capture_callback, &cap);

	send(s, t, FRAMES, RATE, 0.25f);

	assert_int_equal(cap.count, 1);
	assert_int_equal(cap.timestamp, t + MSEC);
	assert_int_equal(cap.frames, 64);
	assert_true(cap.first_left == 0.75f);
	assert_int_equal(s->audio_input_buf[0].size, 64 * sizeof(float));
	assert_int_equal(s->audio_data.frames, FRAMES);
	assert_int_equal(s->audio_data.timestamp, t);

	obs_source_remove_audio_capture_callback(s, capture_callback, &cap);
	obs_source_filter_remove(s, fr);
	obs_source_release(fr);
}

/* NULL arguments and a source being destroyed change nothing. */
static void the_checks_at_the_start_change_nothing(void **state)
{
	struct fixture *f = *state;
	obs_source_t *s = f->source;
	struct obs_source_audio audio = packet(os_gettime_ns(), FRAMES, RATE, 0.25f);

	obs_source_output_audio(NULL, &audio);
	obs_source_output_audio(s, NULL);
	assert_int_equal(s->audio_data.frames, 0);

	s->destroying = true;
	obs_source_output_audio(s, &audio);
	s->destroying = false;
	assert_int_equal(s->audio_data.frames, 0);
	assert_int_equal(s->sample_info.samples_per_sec, 0);

	obs_source_output_audio(s, &audio);
	assert_int_equal(s->audio_data.frames, FRAMES);
}

int main(void)
{
#define CASE(name) cmocka_unit_test_setup_teardown(name, case_setup, case_teardown)
	const struct CMUnitTest tests[] = {
		CASE(a_packet_in_the_output_format_is_copied),
		CASE(the_storage_only_grows),
		CASE(a_format_change_creates_and_removes_the_resampler),
		CASE(a_discontinuity_restarts_the_resampler),
		CASE(the_compensation_flag_creates_and_removes_the_resampler),
		CASE(a_failed_resampler_sends_the_previous_packet_again),
		CASE(unused_planes_are_cleared),
		CASE(the_filters_run_last_to_first),
		CASE(a_filter_can_drop_the_packet),
		CASE(a_filter_can_replace_the_packet),
		CASE(the_checks_at_the_start_change_nothing),
	};
#undef CASE
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
