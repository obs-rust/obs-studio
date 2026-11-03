/* A paired audio encoder starts from its video encoder's start time. The video
 * encoder has two paths that set that time: raw frames (receive_video()) and
 * textures (the GPU encode thread). This test covers the texture path, on
 * libobs-opengl's NV12 textures (software GL is enough): the audio encoder of
 * a texture encoder gets frames (#3571).
 *
 * It skips without a display, and when the core video mix has no texture path.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define TEX_VENC_ID "test_gpu_paired_tex_venc"
#define AENC_ID "test_gpu_paired_aenc"
#define OUTPUT_ID "test_gpu_paired_output"
#define TIMEOUT_MS 10000

static volatile long texture_encodes;
static volatile long audio_encodes;
static uint8_t packet_byte;

static const char *test_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(settings);
	return encoder;
}

static void enc_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool tex_venc_encode(void *data, struct encoder_texture *texture, int64_t pts, uint64_t lock_key,
			    uint64_t *next_key, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(texture);
	UNUSED_PARAMETER(lock_key);
	UNUSED_PARAMETER(next_key);
	os_atomic_inc_long(&texture_encodes);
	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = pts;
	packet->dts = pts;
	packet->type = OBS_ENCODER_VIDEO;
	packet->keyframe = true;
	*received = true;
	return true;
}

static bool aenc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	os_atomic_inc_long(&audio_encodes);
	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->type = OBS_ENCODER_AUDIO;
	*received = true;
	return true;
}

static size_t aenc_frame_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 1024;
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	return output;
}

static void output_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool output_start(void *data)
{
	obs_output_t *output = data;

	return obs_output_can_begin_data_capture(output, 0) && obs_output_initialize_encoders(output, 0) &&
	       obs_output_begin_data_capture(output, 0);
}

static void output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);
	obs_output_end_data_capture(data);
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(packet);
}

static bool wait_counter(volatile long *counter)
{
	for (int i = 0; i < TIMEOUT_MS / 10; i++) {
		if (os_atomic_load_long(counter) > 0)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

static void register_types(void)
{
	struct obs_encoder_info tex = {
		.id = TEX_VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = test_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode_texture2 = tex_venc_encode,
		.caps = OBS_ENCODER_CAP_PASS_TEXTURE,
	};
	obs_register_encoder(&tex);

	struct obs_encoder_info aenc = {
		.id = AENC_ID,
		.type = OBS_ENCODER_AUDIO,
		.codec = "aac",
		.get_name = test_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = aenc_encode,
		.get_frame_size = aenc_frame_size,
	};
	obs_register_encoder(&aenc);

	struct obs_output_info output = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED,
		.get_name = test_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);
}

static void paired_audio_starts_on_the_gpu_path(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		skip();
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.adapter = 0,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_SUCCESS);
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	assert_true(obs_reset_audio(&oai));
	register_types();

	obs_encoder_t *venc = obs_video_encoder_create(TEX_VENC_ID, "video", NULL, NULL);
	obs_encoder_t *aenc = obs_audio_encoder_create(AENC_ID, "audio", NULL, 0, NULL);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "output", NULL, NULL);
	assert_non_null(venc);
	assert_non_null(aenc);
	assert_non_null(output);
	obs_encoder_set_video(venc, obs_get_video());
	obs_encoder_set_audio(aenc, obs_get_audio());
	obs_output_set_video_encoder(output, venc);
	obs_output_set_audio_encoder(output, aenc, 0);

	/* A texture-only encoder is refused without a texture path. */
	bool started = obs_output_start(output);
	bool textures = started && wait_counter(&texture_encodes);
	bool audio = textures && wait_counter(&audio_encodes);

	if (started) {
		obs_output_stop(output);
		for (int i = 0; i < TIMEOUT_MS / 10 && obs_output_active(output); i++)
			os_sleep_ms(10);
	}
	obs_output_release(output);
	obs_encoder_release(aenc);
	obs_encoder_release(venc);
	obs_shutdown();

	if (!textures) {
		fprintf(stderr, "skipped: the core video mix has no texture path\n");
		skip();
	}
	assert_true(audio);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(paired_audio_starts_on_the_gpu_path),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
