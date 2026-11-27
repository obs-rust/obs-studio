/* Direct actual-library characterization of obs_output_set_audio_encoder.
 * Private state arranges gates and observes ownership; all setter calls use
 * loaded libobs. Odd existing behavior is characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include <obs-internal.h>

static obs_output_t *output;
static obs_encoder_t *audio[2];
static obs_encoder_t *video;
static obs_output_t *destroy_target;
static bool destroy_ran;
static audio_t *mix;
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
	return "audio setter characterization";
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

static void encoder_destroy(void *data)
{
	if (destroy_target) {
		obs_output_set_audio_encoder(destroy_target, data, 0);
		destroy_ran = true;
	}
}

static bool mix_input(void *param, uint64_t start, uint64_t end, uint64_t *timestamp, uint32_t mixers,
		      struct audio_output_data *data)
{
	(void)param;
	(void)start;
	(void)end;
	(void)timestamp;
	(void)mixers;
	(void)data;
	return false;
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
		.id = "audio_setter_output",
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK,
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
		.id = "audio_setter_video",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = type_name,
		.create = encoder_create,
		.destroy = encoder_destroy,
		.encode = encode,
	};
	obs_register_encoder(&encoder);
	encoder.id = "audio_setter_audio";
	encoder.type = OBS_ENCODER_AUDIO;
	encoder.codec = "aac";
	encoder.get_frame_size = frame_size;
	obs_register_encoder(&encoder);
	for (size_t i = 0; i < 2; i++) {
		audio[i] = obs_audio_encoder_create("audio_setter_audio", "audio", NULL, 0, NULL);
		assert_non_null(audio[i]);
	}
	video = obs_video_encoder_create("audio_setter_video", "video", NULL, NULL);
	assert_non_null(video);
	struct audio_output_info ai = {.name = "setter mix",
				       .samples_per_sec = 48000,
				       .format = AUDIO_FORMAT_FLOAT_PLANAR,
				       .speakers = SPEAKERS_STEREO,
				       .input_callback = mix_input};
	assert_int_equal(audio_output_open(&mix, &ai), AUDIO_OUTPUT_SUCCESS);
	base_get_log_handler(&previous_log, &previous_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(previous_log, previous_param);
	if (output) {
		os_atomic_set_bool(&output->active, false);
		os_atomic_set_bool(&output->delay_active, false);
	}
	obs_output_release(output);
	for (size_t i = 0; i < 2; i++)
		obs_encoder_release(audio[i]);
	obs_encoder_release(video);
	audio_output_close(mix);
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
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++)
		assert_null(output->audio_encoders[i]);
	for (size_t i = 0; i < 2; i++)
		if (audio[i])
			assert_int_equal(audio[i]->outputs.num, 0);
	assert_int_equal(video->outputs.num, 0);
}

static void null_and_gate_order(void **state)
{
	(void)state;
	obs_output_set_audio_encoder(NULL, video, SIZE_MAX);
	expect_log(LOG_DEBUG, "obs_output_set_audio_encoder: Null 'output' parameter");
	output->info.flags = 0;
	os_atomic_set_bool(&output->active, true);
	obs_output_set_audio_encoder(output, video, SIZE_MAX);
	expect_log(LOG_WARNING, "Output 'setter output': Tried to use obs_output_set_audio_encoder on a raw output");
	output->info.flags = OBS_OUTPUT_ENCODED;
	obs_output_set_audio_encoder(output, video, SIZE_MAX);
	expect_log(LOG_WARNING,
		   "Output 'setter output': Tried to use obs_output_set_audio_encoder on a non-audio output");
	output->info.flags |= OBS_OUTPUT_AUDIO;
	obs_output_set_audio_encoder(output, video, SIZE_MAX);
	expect_log(LOG_WARNING, "obs_output_set_audio_encoder: encoder passed is not an audio encoder");
	obs_output_set_audio_encoder(output, audio[0], 1);
	expect_log(
		LOG_WARNING,
		"obs_output_set_audio_encoder: tried to set audio encoder 1 on output \"setter output\" while the output is still active!");
	assert_empty();
}

static void flags_and_indices(void **state)
{
	(void)state;
	const uint32_t bits[] = {OBS_OUTPUT_ENCODED, OBS_OUTPUT_AUDIO, OBS_OUTPUT_VIDEO, OBS_OUTPUT_MULTI_TRACK,
				 OBS_OUTPUT_MULTI_TRACK_VIDEO};
	const size_t indices[] = {
		0,       1, MAX_OUTPUT_AUDIO_ENCODERS - 1, MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_AUDIO_ENCODERS + 1,
		SIZE_MAX};
	long initial = references(audio[0]);
	for (unsigned mask = 0; mask < 32; mask++) {
		uint32_t flags = 0;
		for (unsigned bit = 0; bit < 5; bit++)
			if (mask & (1U << bit))
				flags |= bits[bit];
		output->info.flags = flags;
		for (size_t n = 0; n < sizeof(indices) / sizeof(indices[0]); n++) {
			size_t idx = indices[n];
			bool accepted = (flags & OBS_OUTPUT_ENCODED) && (flags & OBS_OUTPUT_AUDIO) &&
					idx < MAX_OUTPUT_AUDIO_ENCODERS &&
					(idx == 0 || (flags & OBS_OUTPUT_MULTI_TRACK));
			log_count = 0;
			obs_output_set_audio_encoder(output, audio[0], idx);
			assert_int_equal(references(audio[0]), initial + (accepted ? 1 : 0));
			assert_int_equal(audio[0]->outputs.num, accepted ? 1 : 0);
			assert_int_equal(log_count, (flags & OBS_OUTPUT_ENCODED) && (flags & OBS_OUTPUT_AUDIO) ? 0 : 1);
			for (size_t slot = 0; slot < MAX_OUTPUT_AUDIO_ENCODERS; slot++)
				assert_ptr_equal(output->audio_encoders[slot],
						 accepted && slot == idx ? audio[0] : NULL);
			if (accepted)
				obs_output_set_audio_encoder(output, NULL, idx);
			assert_empty();
		}
	}
}

static void replacement_identity_and_ownership(void **state)
{
	(void)state;
	const size_t idx = MAX_OUTPUT_AUDIO_ENCODERS - 1;
	long first = references(audio[0]);
	long second = references(audio[1]);
	obs_output_set_audio_encoder(output, audio[0], idx);
	assert_int_equal(references(audio[0]), first + 1);
	assert_int_equal(audio[0]->outputs.num, 1);
	assert_ptr_equal(audio[0]->outputs.array[0], output);
	obs_output_set_audio_encoder(output, audio[0], idx);
	assert_int_equal(references(audio[0]), first + 1);
	assert_int_equal(audio[0]->outputs.num, 1);
	obs_output_set_audio_encoder(output, audio[1], idx);
	assert_int_equal(references(audio[0]), first);
	assert_int_equal(audio[0]->outputs.num, 0);
	assert_int_equal(references(audio[1]), second + 1);
	assert_int_equal(audio[1]->outputs.num, 1);
	assert_ptr_equal(audio[1]->outputs.array[0], output);
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(audio[1]);
	obs_encoder_release(audio[1]);
	audio[1] = NULL;
	obs_encoder_t *retained = obs_weak_encoder_get_encoder(weak);
	assert_non_null(retained);
	assert_ptr_equal(retained, output->audio_encoders[idx]);
	obs_encoder_release(retained);
	obs_output_set_audio_encoder(output, NULL, idx);
	assert_null(obs_weak_encoder_get_encoder(weak));
	obs_weak_encoder_release(weak);
	obs_output_set_audio_encoder(output, NULL, idx);
	assert_empty();
	assert_int_equal(log_count, 0);
}

static void active_and_delay_gates(void **state)
{
	(void)state;
	obs_output_set_audio_encoder(output, audio[0], 0);
	long refs = references(audio[0]);
	os_atomic_set_bool(&output->active, true);
	obs_output_set_audio_encoder(output, NULL, 0);
	expect_log(
		LOG_WARNING,
		"obs_output_set_audio_encoder: tried to set audio encoder 0 on output \"setter output\" while the output is still active!");
	obs_output_set_audio_encoder(output, audio[0], 0);
	expect_log(
		LOG_WARNING,
		"obs_output_set_audio_encoder: tried to set audio encoder 0 on output \"setter output\" while the output is still active!");
	assert_ptr_equal(output->audio_encoders[0], audio[0]);
	assert_int_equal(references(audio[0]), refs);
	os_atomic_set_bool(&output->active, false);
	/* Delay alone does not refuse replacement: characterized, not endorsed. */
	os_atomic_set_bool(&output->delay_active, true);
	obs_output_set_audio_encoder(output, audio[1], 0);
	assert_ptr_equal(output->audio_encoders[0], audio[1]);
	assert_int_equal(audio[0]->outputs.num, 0);
	obs_output_set_audio_encoder(output, NULL, 0);
	assert_empty();
	assert_int_equal(log_count, 0);
}

static void sparse_slots_and_output_destruction(void **state)
{
	(void)state;
	long refs = references(audio[0]);
	obs_output_set_audio_encoder(output, audio[0], 1);
	obs_output_set_audio_encoder(output, audio[0], MAX_OUTPUT_AUDIO_ENCODERS - 1);
	assert_null(output->audio_encoders[0]);
	assert_ptr_equal(output->audio_encoders[1], audio[0]);
	assert_ptr_equal(output->audio_encoders[MAX_OUTPUT_AUDIO_ENCODERS - 1], audio[0]);
	/* One reverse entry per slot, including duplicate output pointers. */
	assert_int_equal(audio[0]->outputs.num, 2);
	assert_int_equal(references(audio[0]), refs + 2);
	assert_ptr_equal(audio[0]->outputs.array[0], output);
	assert_ptr_equal(audio[0]->outputs.array[1], output);
	obs_output_set_audio_encoder(output, NULL, 1);
	assert_int_equal(audio[0]->outputs.num, 1);
	assert_int_equal(references(audio[0]), refs + 1);
	obs_output_release(output);
	output = NULL;
	assert_int_equal(audio[0]->outputs.num, 0);
	assert_int_equal(references(audio[0]), refs);
	assert_int_equal(log_count, 0);
}

static void failed_reference_promotion(void **state)
{
	(void)state;
	obs_encoder_set_audio(audio[1], mix);
	obs_output_t *holder = obs_output_create("audio_setter_output", "initializer", NULL, NULL);
	assert_non_null(holder);
	obs_output_set_audio_encoder(holder, audio[1], 0);
	assert_true(obs_output_initialize_encoders(holder, 0));
	obs_output_set_audio_encoder(holder, NULL, 0);
	obs_output_release(holder);
	obs_output_set_audio_encoder(output, audio[0], 0);
	long refs = references(audio[0]);
	destroy_target = output;
	destroy_ran = false;
	obs_encoder_release(audio[1]);
	audio[1] = NULL;
	destroy_target = NULL;
	assert_true(destroy_ran);
	/* The dying encoder's callback passes its still-allocated handle; the old
	 * slot is removed first and failed promotion leaves NULL, without a link. */
	assert_int_equal(references(audio[0]), refs - 1);
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
		cmocka_unit_test_setup_teardown(sparse_slots_and_output_destruction, setup, teardown),
		cmocka_unit_test_setup_teardown(failed_reference_promotion, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
