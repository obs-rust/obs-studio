/* Actual-library characterization: an audio encoder's buffers through
 * libobs's own lifecycle. obs_output_initialize_encoders() initializes the
 * encoder (intitialize_audio_encoder() in obs-encoder-audio-buffers.c.h)
 * and releasing it frees them (free_audio_buffers()), so this runs whichever
 * implementation the loaded libobs has. test_encoder_audio_buffers pins the
 * static C bodies on their own. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#define ENCODER_ID "test_audio_buffers_encoder"
#define OUTPUT_ID "test_audio_buffers_output"

/* What the plugin negotiates and reports. */
struct plugin_case {
	uint32_t rate;
	enum audio_format format;
	enum speaker_layout speakers;
	size_t frames;
	/* What libobs stores from it. */
	size_t planes;
	size_t blocksize;
};

static const struct plugin_case *current;
static int encoder_data;
static int output_data;
static size_t frame_size_calls;

static const char *get_name(void *type_data)
{
	(void)type_data;
	return "audio buffers";
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return &encoder_data;
}

static void encoder_destroy(void *data)
{
	assert_ptr_equal(data, &encoder_data);
}

static bool encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet,
			   bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received_packet = false;
	return true;
}

static size_t encoder_frame_size(void *data)
{
	assert_ptr_equal(data, &encoder_data);
	frame_size_calls++;
	return current->frames;
}

static void encoder_audio_info(void *data, struct audio_convert_info *info)
{
	assert_ptr_equal(data, &encoder_data);
	info->samples_per_sec = current->rate;
	info->format = current->format;
	info->speakers = current->speakers;
}

static struct obs_encoder_info encoder_info = {
	.id = ENCODER_ID,
	.type = OBS_ENCODER_AUDIO,
	.codec = "pcm",
	.get_name = get_name,
	.create = encoder_create,
	.destroy = encoder_destroy,
	.encode = encoder_encode,
	.get_frame_size = encoder_frame_size,
	.get_audio_info = encoder_audio_info,
};

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return &output_data;
}

static void output_destroy(void *data)
{
	assert_ptr_equal(data, &output_data);
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

static struct obs_output_info output_info = {
	.id = OUTPUT_ID,
	.flags = OBS_OUTPUT_AUDIO | OBS_OUTPUT_ENCODED,
	.get_name = get_name,
	.create = output_create,
	.destroy = output_destroy,
	.start = output_start,
	.stop = output_stop,
	.encoded_packet = output_packet,
};

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

static audio_t *mix;
/* obs_startup() needs a display on Linux (hotkeys): without one every
 * test skips. */
static bool started;

/* One encoder and output, initialized and released. */
static void lifecycle(void)
{
	obs_encoder_t *encoder = obs_audio_encoder_create(ENCODER_ID, "audio buffers", NULL, 0, NULL);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "audio buffers", NULL, NULL);
	obs_encoder_set_audio(encoder, mix);
	obs_output_set_audio_encoder(output, encoder, 0);
	obs_output_initialize_encoders(output, 0);
	obs_output_release(output);
	obs_encoder_release(encoder);
}

static const struct plugin_case warm_up = {48000, AUDIO_FORMAT_FLOAT_PLANAR, SPEAKERS_STEREO, 1024, 2, 4};

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}
	started = true;
	struct audio_output_info info = {.name = "audio-buffers",
					 .samples_per_sec = 48000,
					 .format = AUDIO_FORMAT_FLOAT_PLANAR,
					 .speakers = SPEAKERS_STEREO,
					 .input_callback = mix_input};
	if (audio_output_open(&mix, &info) != AUDIO_OUTPUT_SUCCESS)
		return -1;
	obs_register_encoder(&encoder_info);
	obs_register_output(&output_info);
	/* libobs makes some allocations once, on first use: make them before
	 * the tests compare allocation counts. */
	current = &warm_up;
	lifecycle();
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (!started)
		return 0;
	audio_output_close(mix);
	obs_shutdown();
	return 0;
}

static void run_case(const struct plugin_case *c)
{
	if (!started)
		skip();
	current = c;
	frame_size_calls = 0;
	long baseline = bnum_allocs();

	obs_encoder_t *encoder = obs_audio_encoder_create(ENCODER_ID, "audio buffers", NULL, 0, NULL);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "audio buffers", NULL, NULL);
	assert_non_null(encoder);
	assert_non_null(output);
	obs_encoder_set_audio(encoder, mix);
	obs_output_set_audio_encoder(output, encoder, 0);

	/* Nothing is sized or allocated before the encoder is initialized. */
	assert_int_equal(encoder->planes, 0);
	for (size_t i = 0; i < MAX_AV_PLANES; i++)
		assert_null(encoder->audio_output_buffer[i]);

	assert_true(obs_output_initialize_encoders(output, 0));
	assert_int_equal(frame_size_calls, 1);

	assert_int_equal(encoder->samplerate, c->rate);
	assert_int_equal(obs_encoder_get_sample_rate(encoder), c->rate);
	assert_int_equal(encoder->planes, c->planes);
	assert_int_equal(encoder->blocksize, c->blocksize);
	assert_int_equal(encoder->framesize, c->frames);
	assert_int_equal(obs_encoder_get_frame_size(encoder), c->frames);
	assert_int_equal(encoder->framesize_bytes, c->blocksize * c->frames);

	/* One output buffer per active plane, each framesize_bytes long. */
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		if (i < c->planes) {
			assert_non_null(encoder->audio_output_buffer[i]);
			memset(encoder->audio_output_buffer[i], 0x5a, encoder->framesize_bytes);
		} else {
			assert_null(encoder->audio_output_buffer[i]);
		}
	}

	/* Initializing again keeps the same buffers. */
	uint8_t *first = encoder->audio_output_buffer[0];
	assert_true(obs_output_initialize_encoders(output, 0));
	assert_int_equal(frame_size_calls, 1);
	assert_ptr_equal(encoder->audio_output_buffer[0], first);

	obs_output_release(output);
	obs_encoder_release(encoder);

	/* Destroying the encoder freed every buffer it allocated. */
	assert_int_equal(bnum_allocs(), baseline);
}

static void planar_stereo(void **state)
{
	(void)state;
	const struct plugin_case c = {44100, AUDIO_FORMAT_FLOAT_PLANAR, SPEAKERS_STEREO, 1024, 2, 4};
	run_case(&c);
}

static void packed_surround(void **state)
{
	(void)state;
	const struct plugin_case c = {48000, AUDIO_FORMAT_16BIT, SPEAKERS_5POINT1, 960, 1, 12};
	run_case(&c);
}

static void planar_seven_one(void **state)
{
	(void)state;
	const struct plugin_case c = {32000, AUDIO_FORMAT_U8BIT_PLANAR, SPEAKERS_7POINT1, 17, 8, 1};
	run_case(&c);
}

static void packed_mono(void **state)
{
	(void)state;
	const struct plugin_case c = {22050, AUDIO_FORMAT_32BIT, SPEAKERS_MONO, 1, 1, 4};
	run_case(&c);
}

/* Characterized, not endorsed: a planar format with a speaker layout outside
 * the enum has no channels, so no planes and no buffers, though the block
 * size of a planar format is one sample of one channel. */
static void planar_unknown_layout(void **state)
{
	(void)state;
	const struct plugin_case c = {48000, AUDIO_FORMAT_FLOAT_PLANAR, (enum speaker_layout)7, 1024, 0, 4};
	run_case(&c);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(planar_stereo),         cmocka_unit_test(packed_surround),
		cmocka_unit_test(planar_seven_one),      cmocka_unit_test(packed_mono),
		cmocka_unit_test(planar_unknown_layout),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
