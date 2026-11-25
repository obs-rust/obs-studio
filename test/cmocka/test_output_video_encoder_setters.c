/* Direct characterization of the loaded public setters (#5685). No copied setter or
 * Rust stand-in: real output/encoder handles own every reference and caption.
 * Private state only observes effects or arranges flags/active gates without
 * starting capture. Dying-reference cases remain in test_output_slot_stored_encoder. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include <obs-internal.h>

static obs_output_t *output;
static obs_encoder_t *video[2];
static obs_encoder_t *audio;
static log_handler_t previous_log;
static void *previous_param;
static unsigned log_count;
static int log_level;
static char log_text[512];

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	/* Keep NULL admission diagnostics (DEBUG), but ignore lifecycle chatter. */
	if (level > LOG_WARNING && !strstr(format, "Null '"))
		return;
	log_count++;
	log_level = level;
	vsnprintf(log_text, sizeof(log_text), format, args);
}

static const char *type_name(void *data)
{
	(void)data;
	return "video setter characterization";
}

static void *output_create(obs_data_t *settings, obs_output_t *owner)
{
	(void)settings;
	return owner;
}

static void destroy(void *data)
{
	(void)data;
}

static bool start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t timestamp)
{
	(void)data;
	(void)timestamp;
}

static void packet(void *data, struct encoder_packet *value)
{
	(void)data;
	(void)value;
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *owner)
{
	(void)settings;
	return owner;
}

static bool encode(void *data, struct encoder_frame *frame, struct encoder_packet *value, bool *received)
{
	(void)data;
	(void)frame;
	(void)value;
	*received = false;
	return true;
}

static size_t frame_size(void *data)
{
	(void)data;
	return 1024;
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_output_info info = {
		.id = "video_setter_output",
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK_VIDEO,
		.get_name = type_name,
		.create = output_create,
		.destroy = destroy,
		.start = start,
		.stop = output_stop,
		.encoded_packet = packet,
	};
	obs_register_output(&info);
	output = obs_output_create(info.id, "setter output", NULL, NULL);
	assert_non_null(output);
	struct obs_encoder_info encoder = {
		.id = "video_setter_video",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = type_name,
		.create = encoder_create,
		.destroy = destroy,
		.encode = encode,
	};
	obs_register_encoder(&encoder);
	encoder.id = "video_setter_audio";
	encoder.type = OBS_ENCODER_AUDIO;
	encoder.codec = "aac";
	encoder.get_frame_size = frame_size;
	obs_register_encoder(&encoder);
	for (size_t i = 0; i < 2; i++) {
		video[i] = obs_video_encoder_create("video_setter_video", "video", NULL, NULL);
		assert_non_null(video[i]);
	}
	audio = obs_audio_encoder_create("video_setter_audio", "audio", NULL, 0, NULL);
	assert_non_null(audio);
	base_get_log_handler(&previous_log, &previous_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(previous_log, previous_param);
	os_atomic_set_bool(&output->active, false);
	os_atomic_set_bool(&output->delay_active, false);
	obs_output_release(output);
	for (size_t i = 0; i < 2; i++)
		obs_encoder_release(video[i]);
	obs_encoder_release(audio);
	obs_shutdown();
	return 0;
}

static long references(obs_encoder_t *encoder)
{
	return os_atomic_load_long(&encoder->context.control->ref.refs);
}

static void expect_log(int level, const char *text)
{
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, level);
	assert_string_equal(log_text, text);
	log_count = 0;
}

static void assert_empty(void)
{
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		assert_null(output->video_encoders[i]);
		assert_null(output->caption_tracks[i]);
	}
	assert_int_equal(video[0]->outputs.num, 0);
	assert_int_equal(audio->outputs.num, 0);
}

static void null_and_gate_order(void **state)
{
	(void)state;
	obs_output_set_video_encoder(NULL, video[0]);
	expect_log(LOG_DEBUG, "obs_output_set_video_encoder: Null 'output' parameter");
	obs_output_set_video_encoder2(NULL, audio, SIZE_MAX);
	expect_log(LOG_DEBUG, "obs_output_set_video_encoder2: Null 'output' parameter");
	output->info.flags = 0;
	os_atomic_set_bool(&output->active, true);
	obs_output_set_video_encoder2(output, audio, SIZE_MAX);
	expect_log(LOG_WARNING, "Output 'setter output': Tried to use obs_output_set_video_encoder2 on a raw output");
	output->info.flags = OBS_OUTPUT_ENCODED;
	obs_output_set_video_encoder2(output, audio, SIZE_MAX);
	expect_log(LOG_WARNING,
		   "Output 'setter output': Tried to use obs_output_set_video_encoder2 on a non-video output");
	output->info.flags |= OBS_OUTPUT_VIDEO;
	obs_output_set_video_encoder2(output, audio, SIZE_MAX);
	expect_log(LOG_WARNING, "obs_output_set_video_encoder: encoder passed is not a video encoder");
	obs_output_set_video_encoder2(output, video[0], SIZE_MAX);
	expect_log(
		LOG_WARNING,
		"obs_output_set_video_encoder2: tried to set video encoder on output \"setter output\" while the output is still active!");
	assert_empty();
}

static void flags_and_indices(void **state)
{
	(void)state;
	const uint32_t bits[] = {OBS_OUTPUT_ENCODED, OBS_OUTPUT_VIDEO, OBS_OUTPUT_AUDIO, OBS_OUTPUT_MULTI_TRACK_VIDEO};
	const size_t indices[] = {
		0,       1, MAX_OUTPUT_VIDEO_ENCODERS - 1, MAX_OUTPUT_VIDEO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS + 1,
		SIZE_MAX};
	const long initial = references(video[0]);
	for (unsigned mask = 0; mask < 16; mask++) {
		uint32_t flags = 0;
		for (unsigned bit = 0; bit < 4; bit++)
			if (mask & (1U << bit))
				flags |= bits[bit];
		output->info.flags = flags;
		for (size_t n = 0; n < sizeof(indices) / sizeof(indices[0]); n++) {
			size_t idx = indices[n];
			bool accepted = (flags & OBS_OUTPUT_ENCODED) && (flags & OBS_OUTPUT_VIDEO) &&
					idx < MAX_OUTPUT_VIDEO_ENCODERS &&
					(idx == 0 || (flags & OBS_OUTPUT_MULTI_TRACK_VIDEO));
			log_count = 0;
			obs_output_set_video_encoder2(output, video[0], idx);
			assert_int_equal(references(video[0]), initial + (accepted ? 1 : 0));
			assert_int_equal(video[0]->outputs.num, accepted ? 1 : 0);
			assert_int_equal(log_count, (flags & OBS_OUTPUT_ENCODED) && (flags & OBS_OUTPUT_VIDEO) ? 0 : 1);
			for (size_t slot = 0; slot < MAX_OUTPUT_VIDEO_ENCODERS; slot++) {
				assert_ptr_equal(output->video_encoders[slot],
						 accepted && slot == idx ? video[0] : NULL);
				if (accepted && slot == idx)
					assert_non_null(output->caption_tracks[slot]);
				else
					assert_null(output->caption_tracks[slot]);
			}
			if (accepted)
				obs_output_set_video_encoder2(output, NULL, idx);
			assert_empty();
		}
	}
}

static void replacement_identity_and_ownership(void **state)
{
	(void)state;
	const size_t idx = MAX_OUTPUT_VIDEO_ENCODERS - 1;
	long first = references(video[0]);
	long second = references(video[1]);
	obs_output_set_video_encoder2(output, video[0], idx);
	assert_int_equal(references(video[0]), first + 1);
	assert_int_equal(video[0]->outputs.num, 1);
	assert_ptr_equal(video[0]->outputs.array[0], output);
	struct caption_track_data *caption = output->caption_tracks[idx];
	assert_non_null(caption);
	caption->caption_timestamp = 123.5;
	obs_output_set_video_encoder2(output, video[0], idx);
	assert_ptr_equal(output->caption_tracks[idx], caption);
	assert_true(caption->caption_timestamp == 123.5);
	assert_int_equal(references(video[0]), first + 1);
	assert_int_equal(video[0]->outputs.num, 1);
	obs_output_set_video_encoder2(output, video[1], idx);
	assert_int_equal(references(video[0]), first);
	assert_int_equal(video[0]->outputs.num, 0);
	assert_int_equal(references(video[1]), second + 1);
	assert_int_equal(video[1]->outputs.num, 1);
	assert_ptr_equal(video[1]->outputs.array[0], output);
	assert_ptr_equal(output->video_encoders[idx], video[1]);
	assert_non_null(output->caption_tracks[idx]);
	assert_true(output->caption_tracks[idx]->caption_timestamp == 0.0);
	/* Dropping caller ownership leaves the stored strong reference alive. */
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(video[1]);
	obs_encoder_release(video[1]);
	video[1] = NULL;
	obs_encoder_t *retained = obs_weak_encoder_get_encoder(weak);
	assert_non_null(retained);
	assert_ptr_equal(retained, output->video_encoders[idx]);
	obs_encoder_release(retained);
	obs_output_set_video_encoder2(output, NULL, idx);
	assert_null(obs_weak_encoder_get_encoder(weak));
	obs_weak_encoder_release(weak);
	obs_output_set_video_encoder2(output, NULL, idx);
	assert_empty();
	assert_int_equal(log_count, 0);
}

static void active_and_delay_gates(void **state)
{
	(void)state;
	obs_output_set_video_encoder(output, video[0]);
	struct caption_track_data *caption = output->caption_tracks[0];
	long refs = references(video[0]);
	os_atomic_set_bool(&output->active, true);
	obs_output_set_video_encoder(output, NULL);
	expect_log(
		LOG_WARNING,
		"obs_output_set_video_encoder2: tried to set video encoder on output \"setter output\" while the output is still active!");
	obs_output_set_video_encoder2(output, video[0], 0);
	expect_log(
		LOG_WARNING,
		"obs_output_set_video_encoder2: tried to set video encoder on output \"setter output\" while the output is still active!");
	assert_ptr_equal(output->video_encoders[0], video[0]);
	assert_ptr_equal(output->caption_tracks[0], caption);
	assert_int_equal(references(video[0]), refs);
	os_atomic_set_bool(&output->active, false);
	/* Delay alone is not this setter's active gate: characterized, not endorsed. */
	os_atomic_set_bool(&output->delay_active, true);
	obs_output_set_video_encoder(output, video[1]);
	assert_ptr_equal(output->video_encoders[0], video[1]);
	assert_int_equal(video[0]->outputs.num, 0);
	obs_output_set_video_encoder(output, NULL);
	assert_empty();
	assert_int_equal(log_count, 0);
}

static void sparse_slots_and_default_size(void **state)
{
	(void)state;
	video[0]->scaled_width = 320;
	video[0]->scaled_height = 180;
	output->scaled_width = 1280;
	output->scaled_height = 720;
	obs_output_set_video_encoder2(output, video[0], 1);
	assert_int_equal(video[0]->scaled_width, 320);
	assert_int_equal(video[0]->scaled_height, 180);
	assert_null(output->video_encoders[0]);
	obs_output_set_video_encoder(output, video[0]);
	assert_ptr_equal(output->video_encoders[0], video[0]);
	assert_ptr_equal(output->video_encoders[1], video[0]);
	assert_int_equal(video[0]->scaled_width, 1280);
	assert_int_equal(video[0]->scaled_height, 720);
	assert_int_equal(video[0]->outputs.num, 2);
	assert_ptr_equal(video[0]->outputs.array[0], output);
	assert_ptr_equal(video[0]->outputs.array[1], output);
	assert_ptr_not_equal(output->caption_tracks[0], output->caption_tracks[1]);
	/* Same-pointer no-op skips applying a subsequently changed preferred size. */
	output->scaled_width = 640;
	output->scaled_height = 360;
	obs_output_set_video_encoder(output, video[0]);
	assert_int_equal(video[0]->scaled_width, 1280);
	obs_output_set_video_encoder(output, NULL);
	assert_int_equal(video[0]->outputs.num, 1);
	assert_ptr_equal(output->video_encoders[1], video[0]);
	obs_output_set_video_encoder2(output, NULL, 1);
	assert_int_equal(video[0]->outputs.num, 0);
	const uint32_t sizes[][2] = {{0, 0}, {640, 0}, {0, 360}, {640, 360}};
	for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
		output->scaled_width = sizes[i][0];
		output->scaled_height = sizes[i][1];
		video[0]->scaled_width = 320;
		video[0]->scaled_height = 180;
		obs_output_set_video_encoder(output, video[0]);
		bool both = sizes[i][0] && sizes[i][1];
		assert_int_equal(video[0]->scaled_width, both ? 640 : 320);
		assert_int_equal(video[0]->scaled_height, both ? 360 : 180);
		obs_output_set_video_encoder(output, NULL);
	}
	assert_empty();
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_and_gate_order, setup, teardown),
		cmocka_unit_test_setup_teardown(flags_and_indices, setup, teardown),
		cmocka_unit_test_setup_teardown(replacement_identity_and_ownership, setup, teardown),
		cmocka_unit_test_setup_teardown(active_and_delay_gates, setup, teardown),
		cmocka_unit_test_setup_teardown(sparse_slots_and_default_size, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
