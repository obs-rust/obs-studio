/* Characterization of the encoder and output getters in libobs/obs-encoder.c.h
 * and libobs/obs-output.c.h: type lookups by id, defaults and properties,
 * flags, pause state, handlers, counters and the NULL/wrong-type guards.
 *
 * A test video encoder, a test audio encoder, an encoded output and a raw
 * audio output are registered. Nothing is started, so no graphics and no
 * encoding happen. obs_startup() needs a display on Linux (xvfb-run); without
 * one the cases skip. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdio.h>
#include <string.h>

#include <obs.h>

#define VENC_ID "test_getters_venc"
#define AENC_ID "test_getters_aenc"
#define VENC_OTHER_CODEC_ID "test_getters_venc_hevc"
#define ENC_OUTPUT_ID "test_getters_encoded_output"
#define RAW_OUTPUT_ID "test_getters_raw_audio_output"

static bool obs_up;

/* ------------------------------------------------------------------------- */
/* encoders */

static const char *venc_name(void *type_data)
{
	(void)type_data;
	return "getters video encoder";
}

static const char *aenc_name(void *type_data)
{
	(void)type_data;
	return "getters audio encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return bzalloc(1);
}

static void enc_destroy(void *data)
{
	bfree(data);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static size_t aenc_frame_size(void *data)
{
	(void)data;
	return 1024;
}

static void enc_defaults(obs_data_t *settings)
{
	obs_data_set_default_int(settings, "bitrate", 2500);
}

static obs_properties_t *enc_properties(void *data)
{
	(void)data;
	obs_properties_t *props = obs_properties_create();
	obs_properties_add_int(props, "bitrate", "Bitrate", 0, 100000, 1);
	return props;
}

/* ------------------------------------------------------------------------- */
/* outputs */

static const char *out_name(void *type_data)
{
	(void)type_data;
	return "getters output";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return bzalloc(1);
}

static void out_destroy(void *data)
{
	bfree(data);
}

static bool out_start(void *data)
{
	(void)data;
	return false;
}

static void out_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void out_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static void out_raw_audio(void *data, struct audio_data *frames)
{
	(void)data;
	(void)frames;
}

static uint64_t out_total_bytes(void *data)
{
	(void)data;
	return 1234;
}

static void out_defaults(obs_data_t *settings)
{
	obs_data_set_default_string(settings, "path", "default.mkv");
}

static obs_properties_t *out_properties(void *data)
{
	(void)data;
	obs_properties_t *props = obs_properties_create();
	obs_properties_add_text(props, "path", "Path", OBS_TEXT_DEFAULT);
	return props;
}

/* ------------------------------------------------------------------------- */

static int getters_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_encoder_info venc = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = venc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
		.get_defaults = enc_defaults,
		.get_properties = enc_properties,
		.caps = OBS_ENCODER_CAP_DYN_BITRATE,
	};
	obs_register_encoder(&venc);

	struct obs_encoder_info venc_other = venc;
	venc_other.id = VENC_OTHER_CODEC_ID;
	venc_other.codec = "hevc";
	venc_other.get_defaults = NULL;
	venc_other.get_properties = NULL;
	venc_other.caps = 0;
	obs_register_encoder(&venc_other);

	struct obs_encoder_info aenc = {
		.id = AENC_ID,
		.type = OBS_ENCODER_AUDIO,
		.codec = "aac",
		.get_name = aenc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
		.get_frame_size = aenc_frame_size,
	};
	obs_register_encoder(&aenc);

	struct obs_output_info enc_out = {
		.id = ENC_OUTPUT_ID,
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_CAN_PAUSE,
		.get_name = out_name,
		.create = out_create,
		.destroy = out_destroy,
		.start = out_start,
		.stop = out_stop,
		.encoded_packet = out_packet,
		.get_total_bytes = out_total_bytes,
		.get_defaults = out_defaults,
		.get_properties = out_properties,
	};
	obs_register_output(&enc_out);

	struct obs_output_info raw_out = {
		.id = RAW_OUTPUT_ID,
		.flags = OBS_OUTPUT_AUDIO,
		.get_name = out_name,
		.create = out_create,
		.destroy = out_destroy,
		.start = out_start,
		.stop = out_stop,
		.raw_audio = out_raw_audio,
	};
	obs_register_output(&raw_out);
	return 0;
}

static int getters_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

static void need_obs(void)
{
	if (!obs_up)
		skip();
}

/* ------------------------------------------------------------------------- */
/* encoder lookups by id */

static void test_encoder_lookup_by_id(void **state)
{
	(void)state;
	need_obs();

	assert_string_equal(obs_get_encoder_codec(VENC_ID), "h264");
	assert_string_equal(obs_get_encoder_codec(AENC_ID), "aac");
	assert_null(obs_get_encoder_codec("test_getters_missing"));

	assert_int_equal(obs_get_encoder_type(VENC_ID), OBS_ENCODER_VIDEO);
	assert_int_equal(obs_get_encoder_type(AENC_ID), OBS_ENCODER_AUDIO);
	/* characterized, not endorsed: an unknown id reports audio */
	assert_int_equal(obs_get_encoder_type("test_getters_missing"), OBS_ENCODER_AUDIO);

	assert_int_equal(obs_get_encoder_caps(VENC_ID), OBS_ENCODER_CAP_DYN_BITRATE);
	assert_int_equal(obs_get_encoder_caps("test_getters_missing"), 0);

	assert_string_equal(obs_encoder_get_display_name(VENC_ID), "getters video encoder");
	assert_null(obs_encoder_get_display_name("test_getters_missing"));
}

static void test_encoder_defaults_by_id(void **state)
{
	(void)state;
	need_obs();

	obs_data_t *d = obs_encoder_defaults(VENC_ID);
	assert_non_null(d);
	assert_int_equal(obs_data_get_int(d, "bitrate"), 2500);
	obs_data_release(d);

	/* no get_defaults: an empty data object, not NULL */
	d = obs_encoder_defaults(VENC_OTHER_CODEC_ID);
	assert_non_null(d);
	assert_int_equal(obs_data_get_int(d, "bitrate"), 0);
	obs_data_release(d);

	assert_null(obs_encoder_defaults("test_getters_missing"));
}

static void test_encoder_properties_by_id(void **state)
{
	(void)state;
	need_obs();

	obs_properties_t *props = obs_get_encoder_properties(VENC_ID);
	assert_non_null(props);
	assert_non_null(obs_properties_get(props, "bitrate"));
	obs_properties_destroy(props);

	assert_null(obs_get_encoder_properties(VENC_OTHER_CODEC_ID));
	assert_null(obs_get_encoder_properties("test_getters_missing"));
}

/* ------------------------------------------------------------------------- */
/* encoder instance getters */

static void test_video_encoder_getters(void **state)
{
	(void)state;
	need_obs();

	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "venc", NULL, NULL);
	assert_non_null(enc);

	assert_string_equal(obs_encoder_get_id(enc), VENC_ID);
	assert_string_equal(obs_encoder_get_codec(enc), "h264");
	assert_int_equal(obs_encoder_get_type(enc), OBS_ENCODER_VIDEO);
	assert_int_equal(obs_encoder_get_caps(enc), OBS_ENCODER_CAP_DYN_BITRATE);
	assert_int_equal(obs_encoder_get_encoded_frames(enc), 0);
	assert_false(obs_encoder_active(enc));
	assert_false(obs_encoder_paused(enc));

	/* GPU scaling is off by default */
	assert_false(obs_encoder_gpu_scaling_enabled(enc));
	assert_int_equal(obs_encoder_get_scale_type(enc), OBS_SCALE_DISABLE);
	obs_encoder_set_gpu_scale_type(enc, OBS_SCALE_BILINEAR);
	assert_true(obs_encoder_gpu_scaling_enabled(enc));
	assert_int_equal(obs_encoder_get_scale_type(enc), OBS_SCALE_BILINEAR);

	/* a video encoder starts with a frame rate divisor of 1 */
	assert_int_equal(obs_encoder_get_frame_rate_divisor(enc), 1);
	assert_false(obs_encoder_set_frame_rate_divisor(enc, 0));
	assert_int_equal(obs_encoder_get_frame_rate_divisor(enc), 1);
	assert_true(obs_encoder_set_frame_rate_divisor(enc, 2));
	assert_int_equal(obs_encoder_get_frame_rate_divisor(enc), 2);

	/* no media, no scaled size */
	assert_false(obs_encoder_scaling_enabled(enc));
	assert_int_equal(obs_encoder_get_width(enc), 0);
	assert_int_equal(obs_encoder_get_height(enc), 0);

	assert_int_equal(obs_encoder_get_preferred_video_format(enc), VIDEO_FORMAT_NONE);
	obs_encoder_set_preferred_video_format(enc, VIDEO_FORMAT_NV12);
	assert_int_equal(obs_encoder_get_preferred_video_format(enc), VIDEO_FORMAT_NV12);

	/* audio-only getters refuse a video encoder */
	assert_int_equal(obs_encoder_get_sample_rate(enc), 0);
	assert_int_equal(obs_encoder_get_frame_size(enc), 0);
	assert_int_equal(obs_encoder_get_mixer_index(enc), 0);

	obs_data_t *d = obs_encoder_get_defaults(enc);
	assert_non_null(d);
	assert_int_equal(obs_data_get_int(d, "bitrate"), 2500);
	obs_data_release(d);

	d = obs_encoder_get_settings(enc);
	assert_non_null(d);
	assert_int_equal(obs_data_get_int(d, "bitrate"), 2500);
	obs_data_release(d);

	obs_properties_t *props = obs_encoder_properties(enc);
	assert_non_null(props);
	assert_non_null(obs_properties_get(props, "bitrate"));
	obs_properties_destroy(props);

	obs_encoder_release(enc);
}

static void test_encoder_without_properties(void **state)
{
	(void)state;
	need_obs();

	obs_encoder_t *enc = obs_video_encoder_create(VENC_OTHER_CODEC_ID, "venc2", NULL, NULL);
	assert_non_null(enc);
	assert_null(obs_encoder_properties(enc));
	assert_string_equal(obs_encoder_get_codec(enc), "hevc");
	obs_encoder_release(enc);
}

static void test_audio_encoder_getters(void **state)
{
	(void)state;
	need_obs();

	obs_encoder_t *enc = obs_audio_encoder_create(AENC_ID, "aenc", NULL, 3, NULL);
	assert_non_null(enc);

	assert_string_equal(obs_encoder_get_codec(enc), "aac");
	assert_int_equal(obs_encoder_get_type(enc), OBS_ENCODER_AUDIO);
	assert_int_equal(obs_encoder_get_mixer_index(enc), 3);
	/* not initialized, no media */
	assert_int_equal(obs_encoder_get_sample_rate(enc), 0);
	assert_int_equal(obs_encoder_get_frame_size(enc), 0);

	/* video-only getters refuse an audio encoder */
	assert_false(obs_encoder_gpu_scaling_enabled(enc));
	assert_int_equal(obs_encoder_get_scale_type(enc), 0);
	assert_int_equal(obs_encoder_get_frame_rate_divisor(enc), 0);
	assert_false(obs_encoder_set_frame_rate_divisor(enc, 2));
	assert_int_equal(obs_encoder_get_width(enc), 0);
	assert_int_equal(obs_encoder_get_preferred_video_format(enc), VIDEO_FORMAT_NONE);
	obs_encoder_set_preferred_video_format(enc, VIDEO_FORMAT_NV12);
	assert_int_equal(obs_encoder_get_preferred_video_format(enc), VIDEO_FORMAT_NONE);

	obs_encoder_release(enc);
}

static void test_encoder_create_type_mismatch(void **state)
{
	(void)state;
	need_obs();

	/* a registered id of the other type is refused */
	assert_null(obs_video_encoder_create(AENC_ID, "wrong", NULL, NULL));
	assert_null(obs_audio_encoder_create(VENC_ID, "wrong", NULL, 0, NULL));
}

static void test_encoder_create_rerouted_outside_create(void **state)
{
	(void)state;
	need_obs();

	obs_encoder_t *enc = obs_video_encoder_create(VENC_ID, "reroute", NULL, NULL);
	assert_non_null(enc);

	/* rerouting is allowed only from inside create(), during initialize */
	assert_null(obs_encoder_create_rerouted(enc, VENC_ID));
	assert_null(obs_encoder_create_rerouted(enc, NULL));
	assert_string_equal(obs_encoder_get_codec(enc), "h264");

	obs_encoder_release(enc);
	assert_null(obs_encoder_create_rerouted(NULL, VENC_ID));
}

static void test_encoder_null_guards(void **state)
{
	(void)state;
	need_obs();

	assert_null(obs_encoder_get_defaults(NULL));
	assert_null(obs_encoder_properties(NULL));
	assert_null(obs_encoder_get_codec(NULL));
	assert_null(obs_encoder_get_id(NULL));
	assert_null(obs_encoder_get_settings(NULL));
	/* characterized, not endorsed: a NULL encoder reports audio */
	assert_int_equal(obs_encoder_get_type(NULL), OBS_ENCODER_AUDIO);
	assert_int_equal(obs_encoder_get_encoded_frames(NULL), 0);
	assert_int_equal(obs_encoder_get_caps(NULL), 0);
	assert_false(obs_encoder_gpu_scaling_enabled(NULL));
	assert_int_equal(obs_encoder_get_scale_type(NULL), 0);
	assert_int_equal(obs_encoder_get_frame_rate_divisor(NULL), 0);
	assert_false(obs_encoder_scaling_enabled(NULL));
	assert_int_equal(obs_encoder_get_width(NULL), 0);
	assert_int_equal(obs_encoder_get_height(NULL), 0);
	assert_int_equal(obs_encoder_get_sample_rate(NULL), 0);
	assert_int_equal(obs_encoder_get_frame_size(NULL), 0);
	assert_int_equal(obs_encoder_get_mixer_index(NULL), 0);
	assert_int_equal(obs_encoder_get_preferred_video_format(NULL), VIDEO_FORMAT_NONE);
	assert_false(obs_encoder_active(NULL));
	assert_false(obs_encoder_paused(NULL));
}

/* ------------------------------------------------------------------------- */
/* outputs */

static void test_output_lookup_by_id(void **state)
{
	(void)state;
	need_obs();

	assert_int_equal(obs_get_output_flags(ENC_OUTPUT_ID),
			 OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_CAN_PAUSE);
	assert_int_equal(obs_get_output_flags(RAW_OUTPUT_ID), OBS_OUTPUT_AUDIO);
	assert_int_equal(obs_get_output_flags("test_getters_missing"), 0);

	assert_string_equal(obs_output_get_display_name(ENC_OUTPUT_ID), "getters output");
	assert_null(obs_output_get_display_name("test_getters_missing"));

	obs_data_t *d = obs_output_defaults(ENC_OUTPUT_ID);
	assert_non_null(d);
	assert_string_equal(obs_data_get_string(d, "path"), "default.mkv");
	obs_data_release(d);
	d = obs_output_defaults(RAW_OUTPUT_ID);
	assert_non_null(d);
	obs_data_release(d);
	assert_null(obs_output_defaults("test_getters_missing"));

	obs_properties_t *props = obs_get_output_properties(ENC_OUTPUT_ID);
	assert_non_null(props);
	assert_non_null(obs_properties_get(props, "path"));
	obs_properties_destroy(props);
	assert_null(obs_get_output_properties(RAW_OUTPUT_ID));
	assert_null(obs_get_output_properties("test_getters_missing"));
}

static void test_encoded_output_getters(void **state)
{
	(void)state;
	need_obs();

	obs_output_t *out = obs_output_create(ENC_OUTPUT_ID, "enc out", NULL, NULL);
	assert_non_null(out);

	assert_string_equal(obs_output_get_id(out), ENC_OUTPUT_ID);
	assert_int_equal(obs_output_get_flags(out), OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_CAN_PAUSE);
	assert_true(obs_output_can_pause(out));
	assert_false(obs_output_paused(out));
	assert_int_equal(obs_output_get_pause_offset(out), 0);
	assert_false(obs_output_active(out));
	assert_false(obs_output_reconnecting(out));
	assert_non_null(obs_output_get_proc_handler(out));
	assert_non_null(obs_output_get_signal_handler(out));

	/* the type's callback, not delayed */
	assert_int_equal(obs_output_get_total_bytes(out), 1234);
	assert_int_equal(obs_output_get_total_frames(out), 0);
	assert_int_equal(obs_output_get_frames_dropped(out), 0);
	assert_int_equal(obs_output_get_connect_time_ms(out), -1);
	assert_true(obs_output_get_congestion(out) == 0.0f);

	assert_null(obs_output_get_last_error(out));
	obs_output_set_last_error(out, "boom");
	assert_string_equal(obs_output_get_last_error(out), "boom");
	obs_output_set_last_error(out, NULL);
	assert_null(obs_output_get_last_error(out));

	/* raw-only setters are ignored on an encoded output */
	obs_output_set_mixers(out, 0x5);
	assert_int_equal(obs_output_get_mixers(out), 0);
	obs_output_set_mixer(out, 2);
	assert_int_equal(obs_output_get_mixer(out), 0);
	struct audio_convert_info conv = {
		.samples_per_sec = 48000,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.speakers = SPEAKERS_STEREO,
	};
	obs_output_set_audio_conversion(out, &conv);

	obs_data_t *d = obs_output_get_settings(out);
	assert_non_null(d);
	assert_string_equal(obs_data_get_string(d, "path"), "default.mkv");
	obs_data_release(d);

	obs_properties_t *props = obs_output_properties(out);
	assert_non_null(props);
	assert_non_null(obs_properties_get(props, "path"));
	obs_properties_destroy(props);

	obs_output_release(out);
}

static void test_raw_output_getters(void **state)
{
	(void)state;
	need_obs();

	obs_output_t *out = obs_output_create(RAW_OUTPUT_ID, "raw out", NULL, NULL);
	assert_non_null(out);

	assert_false(obs_output_can_pause(out));
	assert_int_equal(obs_output_get_total_bytes(out), 0);
	assert_null(obs_output_properties(out));

	assert_int_equal(obs_output_get_mixers(out), 0);
	obs_output_set_mixer(out, 2);
	assert_int_equal(obs_output_get_mixers(out), 1 << 2);
	assert_int_equal(obs_output_get_mixer(out), 2);
	/* out of range: ignored */
	obs_output_set_mixer(out, MAX_AUDIO_MIXES);
	assert_int_equal(obs_output_get_mixers(out), 1 << 2);
	obs_output_set_mixers(out, 0x6);
	assert_int_equal(obs_output_get_mixers(out), 0x6);
	assert_int_equal(obs_output_get_mixer(out), 1);

	struct audio_convert_info conv = {
		.samples_per_sec = 48000,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.speakers = SPEAKERS_STEREO,
	};
	obs_output_set_audio_conversion(out, &conv);
	obs_output_set_audio_conversion(out, NULL);

	obs_output_release(out);
}

static void test_output_null_guards(void **state)
{
	(void)state;
	need_obs();

	assert_int_equal(obs_output_get_flags(NULL), 0);
	assert_null(obs_output_properties(NULL));
	assert_false(obs_output_can_pause(NULL));
	assert_false(obs_output_paused(NULL));
	assert_int_equal(obs_output_get_pause_offset(NULL), 0);
	assert_null(obs_output_get_proc_handler(NULL));
	assert_null(obs_output_get_signal_handler(NULL));
	assert_int_equal(obs_output_get_total_bytes(NULL), 0);
	assert_int_equal(obs_output_get_total_frames(NULL), 0);
	assert_int_equal(obs_output_get_frames_dropped(NULL), 0);
	assert_int_equal(obs_output_get_connect_time_ms(NULL), -1);
	assert_null(obs_output_get_last_error(NULL));
	assert_null(obs_output_get_id(NULL));
	assert_null(obs_output_get_settings(NULL));
	assert_int_equal(obs_output_get_mixers(NULL), 0);
	assert_int_equal(obs_output_get_mixer(NULL), 0);
	assert_false(obs_output_active(NULL));
	assert_false(obs_output_reconnecting(NULL));
	obs_output_set_audio_conversion(NULL, NULL);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_encoder_lookup_by_id),
		cmocka_unit_test(test_encoder_defaults_by_id),
		cmocka_unit_test(test_encoder_properties_by_id),
		cmocka_unit_test(test_video_encoder_getters),
		cmocka_unit_test(test_encoder_without_properties),
		cmocka_unit_test(test_audio_encoder_getters),
		cmocka_unit_test(test_encoder_create_type_mismatch),
		cmocka_unit_test(test_encoder_create_rerouted_outside_create),
		cmocka_unit_test(test_encoder_null_guards),
		cmocka_unit_test(test_output_lookup_by_id),
		cmocka_unit_test(test_encoded_output_getters),
		cmocka_unit_test(test_raw_output_getters),
		cmocka_unit_test(test_output_null_guards),
	};
	return cmocka_run_group_tests(tests, getters_setup, getters_teardown);
}
