/* Actual-libobs characterization of the complete public initializer. No frames,
 * graphics device or hardware codec: bare media queues and synthetic plugins.
 * Internal fields set up the active/delay states without starting capture. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#define VIDEO_ID "test_init_video"
#define AUDIO_ID "test_init_audio"
#define OUTPUT_ID "test_init_output"
#define RAW_ID "test_init_raw"

struct fixture {
	obs_output_t *output;
	obs_encoder_t *encoders[4];
	video_t *video;
	audio_t *audio;
};

static unsigned trace[4];
static size_t trace_count;
static unsigned fail_marker;
static bool empty_error;
static unsigned destroys;

static const char *plugin_name(void *data)
{
	(void)data;
	return "initialize encoders characterization";
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	unsigned marker = (unsigned)obs_data_get_int(settings, "marker");
	assert_true(trace_count < 4);
	trace[trace_count++] = marker;
	if (marker == fail_marker) {
		if (!empty_error)
			obs_encoder_set_last_error(encoder, "synthetic initialization failure");
		return NULL;
	}
	return encoder;
}

static void encoder_destroy(void *data)
{
	assert_non_null(data);
	destroys++;
}

static bool encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static size_t frame_size(void *data)
{
	(void)data;
	return 1024;
}

static bool mix_input(void *data, uint64_t start, uint64_t end, uint64_t *timestamp, uint32_t mixers,
		      struct audio_output_data *mixes)
{
	(void)data;
	(void)start;
	(void)end;
	(void)timestamp;
	(void)mixers;
	(void)mixes;
	return false;
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void output_destroy(void *data)
{
	(void)data;
}

static bool output_start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t timestamp)
{
	(void)data;
	(void)timestamp;
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static void raw_video(void *data, struct video_data *frame)
{
	(void)data;
	(void)frame;
}

static int group_setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_encoder_info info = {.id = VIDEO_ID,
					.type = OBS_ENCODER_VIDEO,
					.codec = "h264",
					.get_name = plugin_name,
					.create = encoder_create,
					.destroy = encoder_destroy,
					.encode = encoder_encode};
	obs_register_encoder(&info);
	info.id = AUDIO_ID;
	info.type = OBS_ENCODER_AUDIO;
	info.codec = "pcm";
	info.get_frame_size = frame_size;
	obs_register_encoder(&info);
	struct obs_output_info output = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK | OBS_OUTPUT_MULTI_TRACK_VIDEO,
		.get_name = plugin_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
		.raw_video = raw_video,
	};
	obs_register_output(&output);
	output.id = RAW_ID;
	output.flags = OBS_OUTPUT_VIDEO;
	obs_register_output(&output);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static struct fixture fixture;

static int setup(void **state)
{
	memset(&fixture, 0, sizeof(fixture));
	*state = &fixture;
	trace_count = destroys = fail_marker = 0;
	empty_error = false;
	fixture.output = obs_output_create(OUTPUT_ID, "init output", NULL, NULL);
	assert_non_null(fixture.output);
	struct video_output_info video = {.name = "init video",
					  .format = VIDEO_FORMAT_I420,
					  .fps_num = 30,
					  .fps_den = 1,
					  .width = 64,
					  .height = 64,
					  .cache_size = 2,
					  .colorspace = VIDEO_CS_709,
					  .range = VIDEO_RANGE_PARTIAL};
	assert_int_equal(video_output_open(&fixture.video, &video), VIDEO_OUTPUT_SUCCESS);
	struct audio_output_info audio = {.name = "init audio",
					  .samples_per_sec = 48000,
					  .format = AUDIO_FORMAT_FLOAT_PLANAR,
					  .speakers = SPEAKERS_STEREO,
					  .input_callback = mix_input};
	assert_int_equal(audio_output_open(&fixture.audio, &audio), AUDIO_OUTPUT_SUCCESS);
	for (unsigned i = 0; i < 4; i++) {
		obs_data_t *settings = obs_data_create();
		obs_data_set_int(settings, "marker", i + 1);
		fixture.encoders[i] = i < 2 ? obs_video_encoder_create(VIDEO_ID, "init video", settings, NULL)
					    : obs_audio_encoder_create(AUDIO_ID, "init audio", settings, 0, NULL);
		obs_data_release(settings);
		assert_non_null(fixture.encoders[i]);
		if (i < 2)
			obs_encoder_set_video(fixture.encoders[i], fixture.video);
		else
			obs_encoder_set_audio(fixture.encoders[i], fixture.audio);
	}
	/* Sparse slots, including the last slot, expose holes and loop bounds. */
	obs_output_set_video_encoder2(fixture.output, fixture.encoders[0], 1);
	obs_output_set_video_encoder2(fixture.output, fixture.encoders[1], MAX_OUTPUT_VIDEO_ENCODERS - 1);
	obs_output_set_audio_encoder(fixture.output, fixture.encoders[2], 1);
	obs_output_set_audio_encoder(fixture.output, fixture.encoders[3], MAX_OUTPUT_AUDIO_ENCODERS - 1);
	assert_ptr_equal(fixture.output->video_encoders[1], fixture.encoders[0]);
	assert_ptr_equal(fixture.output->video_encoders[MAX_OUTPUT_VIDEO_ENCODERS - 1], fixture.encoders[1]);
	assert_ptr_equal(fixture.output->audio_encoders[1], fixture.encoders[2]);
	assert_ptr_equal(fixture.output->audio_encoders[MAX_OUTPUT_AUDIO_ENCODERS - 1], fixture.encoders[3]);
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	/* These test-only state bits must not send destroy down a capture path. */
	os_atomic_set_bool(&f->output->active, false);
	os_atomic_set_bool(&f->output->delay_active, false);
	obs_output_release(f->output);
	for (size_t i = 0; i < 4; i++)
		obs_encoder_release(f->encoders[i]);
	video_output_close(f->video);
	audio_output_close(f->audio);
	return 0;
}

static void assert_trace(const unsigned *expected, size_t count)
{
	assert_int_equal(trace_count, count);
	if (count)
		assert_memory_equal(trace, expected, count * sizeof(*trace));
}

static void null_and_raw_refuse(void **state)
{
	(void)state;
	assert_false(obs_output_initialize_encoders(NULL, UINT32_MAX));
	obs_output_t *raw = obs_output_create(RAW_ID, "raw init", NULL, NULL);
	assert_non_null(raw);
	obs_output_set_last_error(raw, "keep raw error");
	os_atomic_set_bool(&raw->active, true);
	os_atomic_set_bool(&raw->delay_active, true);
	/* Raw refusal precedes the active+delay shortcut. */
	assert_false(obs_output_initialize_encoders(raw, 0));
	assert_string_equal(obs_output_get_last_error(raw), "keep raw error");
	os_atomic_set_bool(&raw->active, false);
	os_atomic_set_bool(&raw->delay_active, false);
	obs_output_release(raw);
	assert_trace(NULL, 0);
}

static void active_uses_only_delay(void **state)
{
	struct fixture *f = *state;
	obs_output_set_last_error(f->output, "keep active error");
	os_atomic_set_bool(&f->output->active, true);
	for (unsigned delay = 0; delay < 2; delay++) {
		os_atomic_set_bool(&f->output->delay_active, delay != 0);
		assert_int_equal(obs_output_initialize_encoders(f->output, UINT32_MAX), delay != 0);
		assert_string_equal(obs_output_get_last_error(f->output), "keep active error");
		assert_trace(NULL, 0);
		for (size_t i = 0; i < 4; i++)
			assert_false(f->encoders[i]->initialized);
	}
}

static void sparse_order_and_ignored_flags(void **state)
{
	struct fixture *f = *state;
	const unsigned expected[] = {1, 2, 3, 4};
	obs_output_set_last_error(f->output, "keep successful error");
	/* Characterized, not endorsed: flags neither select nor mask encoders. */
	assert_true(obs_output_initialize_encoders(f->output, OBS_OUTPUT_AUDIO));
	assert_trace(expected, 4);
	for (size_t i = 0; i < 4; i++)
		assert_true(f->encoders[i]->initialized);
	assert_string_equal(obs_output_get_last_error(f->output), "keep successful error");
	assert_true(obs_output_initialize_encoders(f->output, UINT32_MAX));
	assert_trace(expected, 4);
	assert_int_equal(destroys, 0);
}

static void inactive_delay_still_initializes(void **state)
{
	struct fixture *f = *state;
	os_atomic_set_bool(&f->output->delay_active, true);
	const unsigned expected[] = {1, 2, 3, 4};
	assert_true(obs_output_initialize_encoders(f->output, 0));
	assert_trace(expected, 4);
}

static void output_flags_select_types(void **state)
{
	struct fixture *f = *state;
	/* Change only the advertised type bits on this synthetic output; keep
	 * its real encoder slots so an unwanted traversal is observable. */
	f->output->info.flags = OBS_OUTPUT_ENCODED;
	assert_true(obs_output_initialize_encoders(f->output, OBS_OUTPUT_AV));
	assert_trace(NULL, 0);
	f->output->info.flags |= OBS_OUTPUT_AUDIO;
	const unsigned audio[] = {3, 4};
	assert_true(obs_output_initialize_encoders(f->output, OBS_OUTPUT_VIDEO));
	assert_trace(audio, 2);
	trace_count = 0;
	f->output->info.flags = OBS_OUTPUT_ENCODED | OBS_OUTPUT_VIDEO;
	const unsigned video[] = {1, 2};
	assert_true(obs_output_initialize_encoders(f->output, OBS_OUTPUT_AUDIO));
	assert_trace(video, 2);
}

static void run_failure(struct fixture *f, unsigned marker, bool no_error)
{
	fail_marker = marker;
	empty_error = no_error;
	obs_output_set_last_error(f->output, "old output error");
	const unsigned expected[] = {1, 2, 3, 4};
	assert_false(obs_output_initialize_encoders(f->output, 0));
	assert_trace(expected, marker);
	for (unsigned i = 0; i < 4; i++)
		assert_int_equal(f->encoders[i]->initialized, i + 1 < marker);
	/* Earlier successful encoders are retained, later ones never visited. */
	assert_int_equal(destroys, 0);
	if (no_error) {
		/* Characterized, not endorsed: a NULL encoder error clears the old one. */
		assert_null(obs_output_get_last_error(f->output));
	} else {
		assert_string_equal(obs_output_get_last_error(f->output), "synthetic initialization failure");
		assert_ptr_not_equal(obs_output_get_last_error(f->output),
				     obs_encoder_get_last_error(f->encoders[marker - 1]));
		obs_encoder_set_last_error(f->encoders[marker - 1], NULL);
		assert_string_equal(obs_output_get_last_error(f->output), "synthetic initialization failure");
	}
	/* Retry skips previously initialized slots, then resumes in slot order. */
	fail_marker = 0;
	trace_count = 0;
	assert_true(obs_output_initialize_encoders(f->output, UINT32_MAX));
	assert_trace(expected + marker - 1, 5 - marker);
}

static void first_video_failure(void **state)
{
	run_failure(*state, 1, false);
}

static void last_video_failure(void **state)
{
	run_failure(*state, 2, false);
}

static void first_audio_failure(void **state)
{
	run_failure(*state, 3, false);
}

static void last_audio_failure(void **state)
{
	run_failure(*state, 4, false);
}

static void failure_without_error(void **state)
{
	run_failure(*state, 3, true);
}

static void empty_slots_succeed(void **state)
{
	struct fixture *f = *state;
	obs_output_set_video_encoder2(f->output, NULL, 1);
	obs_output_set_video_encoder2(f->output, NULL, MAX_OUTPUT_VIDEO_ENCODERS - 1);
	obs_output_set_audio_encoder(f->output, NULL, 1);
	obs_output_set_audio_encoder(f->output, NULL, MAX_OUTPUT_AUDIO_ENCODERS - 1);
	obs_output_set_last_error(f->output, "keep empty error");
	assert_true(obs_output_initialize_encoders(f->output, UINT32_MAX));
	assert_trace(NULL, 0);
	assert_string_equal(obs_output_get_last_error(f->output), "keep empty error");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_and_raw_refuse, setup, teardown),
		cmocka_unit_test_setup_teardown(active_uses_only_delay, setup, teardown),
		cmocka_unit_test_setup_teardown(sparse_order_and_ignored_flags, setup, teardown),
		cmocka_unit_test_setup_teardown(inactive_delay_still_initializes, setup, teardown),
		cmocka_unit_test_setup_teardown(output_flags_select_types, setup, teardown),
		cmocka_unit_test_setup_teardown(first_video_failure, setup, teardown),
		cmocka_unit_test_setup_teardown(last_video_failure, setup, teardown),
		cmocka_unit_test_setup_teardown(first_audio_failure, setup, teardown),
		cmocka_unit_test_setup_teardown(last_audio_failure, setup, teardown),
		cmocka_unit_test_setup_teardown(failure_without_error, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_slots_succeed, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
