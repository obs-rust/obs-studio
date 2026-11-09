/* Characterize reroute admission and plugin selection through real libobs.
 * No encoding or graphics device is needed: initialize through an output
 * attached to a bare video queue, without starting capture. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

#define ORIGINAL "test_reroute_original"
#define REPLACEMENT "test_reroute_replacement"
#define OTHER_TYPE "test_reroute_audio"
#define OTHER_CODEC "test_reroute_hevc"

static bool callback_fails;
static unsigned replacement_calls;
static unsigned destroy_calls;
static obs_data_t *current_settings;
static obs_encoder_t *current_encoder;

static const char *encoder_name(void *data)
{
	(void)data;
	return "reroute characterization";
}

static void *replacement_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	replacement_calls++;
	assert_ptr_equal(settings, current_settings);
	assert_ptr_equal(encoder, current_encoder);
	assert_int_equal(obs_data_get_int(settings, "marker"), 73);
	/* The whole active table is installed before the callback runs. */
	assert_string_equal(encoder->info.id, REPLACEMENT);
	assert_string_equal(encoder->info.codec, "H264");
	assert_int_equal(encoder->info.caps, OBS_ENCODER_CAP_DYN_BITRATE);
	assert_ptr_equal(encoder->info.create, replacement_create);
	assert_string_equal(encoder->orig_info.id, ORIGINAL);
	assert_string_equal(encoder->orig_info.codec, "h264");
	return callback_fails ? NULL : encoder;
}

static void *other_thread(void *data)
{
	/* Admission belongs to the initializing thread, not the encoder. */
	return obs_encoder_create_rerouted(data, REPLACEMENT);
}

static void *original_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	current_settings = settings;
	current_encoder = encoder;
	struct obs_encoder_info before = encoder->info;
	assert_null(obs_encoder_create_rerouted(NULL, REPLACEMENT));
	assert_null(obs_encoder_create_rerouted(encoder, NULL));
	assert_null(obs_encoder_create_rerouted(encoder, ""));
	assert_null(obs_encoder_create_rerouted(encoder, "test_reroute_missing"));
	assert_null(obs_encoder_create_rerouted(encoder, OTHER_TYPE));
	assert_null(obs_encoder_create_rerouted(encoder, OTHER_CODEC));
	assert_memory_equal(&encoder->info, &before, sizeof(before));
	assert_int_equal(replacement_calls, 0);
	pthread_t thread;
	void *result = encoder;
	assert_int_equal(pthread_create(&thread, NULL, other_thread, encoder), 0);
	assert_int_equal(pthread_join(thread, &result), 0);
	assert_null(result);
	assert_int_equal(replacement_calls, 0);
	return obs_encoder_create_rerouted(encoder, REPLACEMENT);
}

static void encoder_destroy(void *data)
{
	assert_ptr_equal(data, current_encoder);
	destroy_calls++;
}

static bool encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static size_t audio_frame_size(void *data)
{
	(void)data;
	return 1024;
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

static void output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_encoder_info info = {
		.id = ORIGINAL,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = encoder_name,
		.create = original_create,
		.destroy = encoder_destroy,
		.encode = encoder_encode,
	};
	obs_register_encoder(&info);
	info.id = REPLACEMENT;
	info.codec = "H264";
	info.create = replacement_create;
	info.caps = OBS_ENCODER_CAP_DYN_BITRATE;
	obs_register_encoder(&info);
	info.id = OTHER_CODEC;
	info.codec = "hevc";
	obs_register_encoder(&info);
	info.id = OTHER_TYPE;
	info.codec = "h264";
	info.type = OBS_ENCODER_AUDIO;
	info.get_frame_size = audio_frame_size;
	obs_register_encoder(&info);
	struct obs_output_info output = {
		.id = "test_reroute_output",
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
		.get_name = encoder_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void run_reroute(bool fail)
{
	callback_fails = fail;
	replacement_calls = destroy_calls = 0;
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "marker", 73);
	obs_encoder_t *encoder = obs_video_encoder_create(ORIGINAL, "reroute", settings, NULL);
	obs_data_release(settings);
	assert_non_null(encoder);
	assert_null(obs_encoder_create_rerouted(encoder, REPLACEMENT));
	assert_string_equal(encoder->info.id, ORIGINAL);
	struct video_output_info voi = {
		.name = "reroute video",
		.format = VIDEO_FORMAT_I420,
		.fps_num = 30,
		.fps_den = 1,
		.width = 64,
		.height = 64,
		.cache_size = 2,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
	};
	video_t *video = NULL;
	assert_int_equal(video_output_open(&video, &voi), VIDEO_OUTPUT_SUCCESS);
	obs_encoder_set_video(encoder, video);
	obs_output_t *output = obs_output_create("test_reroute_output", "reroute output", NULL, NULL);
	assert_non_null(output);
	obs_output_set_video_encoder(output, encoder);
	assert_int_equal(obs_output_initialize_encoders(output, 0), !fail);
	assert_int_equal(replacement_calls, 1);
	/* Characterized, not endorsed: a failed replacement leaves its info
	 * installed; neither the original info nor the active table rolls back. */
	assert_string_equal(encoder->info.id, REPLACEMENT);
	assert_string_equal(encoder->orig_info.id, ORIGINAL);
	assert_ptr_equal(encoder->context.data, fail ? NULL : encoder);
	struct obs_encoder_info before = encoder->info;
	assert_null(obs_encoder_create_rerouted(encoder, REPLACEMENT));
	assert_memory_equal(&encoder->info, &before, sizeof(before));
	assert_int_equal(replacement_calls, 1);
	obs_output_release(output);
	obs_encoder_release(encoder);
	video_output_close(video);
	assert_int_equal(destroy_calls, fail ? 0 : 1);
}

static void successful_replacement(void **state)
{
	(void)state;
	run_reroute(false);
}

static void failed_replacement(void **state)
{
	(void)state;
	run_reroute(true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(successful_replacement),
		cmocka_unit_test(failed_replacement),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
