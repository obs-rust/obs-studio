#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <errno.h>
#include <stdlib.h>
#include <string.h>

#include "../../plugins/obs-ffmpeg/obs-ffmpeg-video-encoders.h"

static int init_codec_calls;
static int set_options_error;
static char last_error[256];

static bool test_init_codec(struct ffmpeg_video_encoder *enc)
{
	UNUSED_PARAMETER(enc);
	init_codec_calls++;
	return true;
}

static int test_set_dict(void *obj, const char *name, const AVDictionary *value, int flags)
{
	return set_options_error ? set_options_error : av_opt_set_dict_val(obj, name, value, flags);
}

/* Exercise the private production update function without opening a codec
 * until its configuration has been inspected. The smoke test opens it below. */
#define ffmpeg_video_encoder_init_codec test_init_codec
#define av_opt_set_dict_val test_set_dict
#include "../../plugins/obs-ffmpeg/obs-ffmpeg-av1.c"
#undef av_opt_set_dict_val
#undef ffmpeg_video_encoder_init_codec

/* Deterministic metadata replaces a live OBS encoder/graphics session. */
video_t *svt_test_encoder_video(const obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(encoder);
	return NULL;
}

const struct video_output_info *svt_test_video_info(const video_t *video)
{
	UNUSED_PARAMETER(video);
	static const struct video_output_info info = {
		.format = VIDEO_FORMAT_I420,
		.fps_num = 30,
		.fps_den = 1,
		.width = 128,
		.height = 128,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
	};
	return &info;
}

uint32_t svt_test_encoder_size(const obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(encoder);
	return 128;
}

const char *svt_test_encoder_name(const obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(encoder);
	return "test";
}

void svt_test_set_last_error(obs_encoder_t *encoder, const char *message)
{
	UNUSED_PARAMETER(encoder);
	snprintf(last_error, sizeof(last_error), "%s", message ? message : "");
}

const char *obs_module_text(const char *text)
{
	if (strcmp(text, "Encoder.Error") == 0)
		return "Failed to open %1: %2";
	return text;
}

struct test_encoder {
	struct av1_encoder enc;
	obs_data_t *settings;
	AVDictionary *options;
};

static int setup(void **state)
{
	struct test_encoder *test = bzalloc(sizeof(*test));
	*state = test;
	test->enc.type = AV1_ENCODER_TYPE_SVT;
	test->enc.ffve.enc_name = "SVT-AV1";
	test->enc.ffve.avcodec = avcodec_find_encoder_by_name("libsvtav1");
	test->enc.ffve.context = avcodec_alloc_context3(test->enc.ffve.avcodec);
	assert_non_null(test->enc.ffve.context);
	test->settings = obs_data_create();
	av1_defaults(test->settings);
	init_codec_calls = 0;
	set_options_error = 0;
	last_error[0] = '\0';
	return 0;
}

static int teardown(void **state)
{
	struct test_encoder *test = *state;
	av_dict_free(&test->options);
	avcodec_free_context(&test->enc.ffve.context);
	av_frame_free(&test->enc.ffve.vframe);
	obs_data_release(test->settings);
	bfree(test);
	return 0;
}

static void check_rate_control(struct test_encoder *test, const char *mode, const char *expected_rc,
			       int64_t expected_bitrate)
{
	obs_data_set_string(test->settings, "rate_control", mode);
	assert_true(av1_update(&test->enc, test->settings));
	assert_int_equal(init_codec_calls, 1);
	AVCodecContext *context = test->enc.ffve.context;
	assert_int_equal(context->bit_rate, expected_bitrate);
	assert_int_equal(context->rc_buffer_size, expected_bitrate);
	assert_int_equal(context->rc_max_rate, 0);
	assert_int_equal(av_opt_get_dict_val(context->priv_data, "svtav1-params", 0, &test->options), 0);
	const AVDictionaryEntry *rc = av_dict_get(test->options, "rc", NULL, 0);
	assert_non_null(rc);
	assert_string_equal(rc->value, expected_rc);
	/* The target bitrate comes from bit_rate (bps), not SVT's tbr (kbps). */
	/* bias-pct is no longer accepted by current SVT versions. */
	assert_null(av_dict_get(test->options, "tbr", NULL, 0));
	assert_null(av_dict_get(test->options, "bias-pct", NULL, 0));
}

static void svt_cbr(void **state)
{
	struct test_encoder *test = *state;
	check_rate_control(test, "CBR", "2", 6000000);
	assert_int_equal(test->enc.ffve.context->rc_min_rate, 6000000);
	const AVDictionaryEntry *prediction = av_dict_get(test->options, "pred-struct", NULL, 0);
	assert_non_null(prediction);
	assert_string_equal(prediction->value, "1");
}

static void svt_vbr(void **state)
{
	struct test_encoder *test = *state;
	check_rate_control(test, "vBr", "1", 6000000);
	assert_null(av_dict_get(test->options, "pred-struct", NULL, 0));
}

static void svt_cqp(void **state)
{
	struct test_encoder *test = *state;
	check_rate_control(test, "CQP", "0", 0);
	int64_t quality = 0;
	assert_int_equal(av_opt_get_int(test->enc.ffve.context->priv_data, "crf", 0, &quality), 0);
	assert_int_equal(quality, 50);
	assert_int_equal(av_opt_get_int(test->enc.ffve.context->priv_data, "qp", 0, &quality), 0);
	assert_int_equal(quality, 50);
}

static void svt_custom_options(void **state)
{
	struct test_encoder *test = *state;
	obs_data_set_string(test->settings, "ffmpeg_opts", "svtav1-params=rc=1:pred-struct=2");
	check_rate_control(test, "CBR", "1", 6000000);
	const AVDictionaryEntry *prediction = av_dict_get(test->options, "pred-struct", NULL, 0);
	assert_non_null(prediction);
	assert_string_equal(prediction->value, "2");
}

static void svt_custom_tuning_keeps_cbr(void **state)
{
	struct test_encoder *test = *state;
	obs_data_set_string(test->settings, "ffmpeg_opts", "svtav1-params=tune=0");
	check_rate_control(test, "CBR", "2", 6000000);
	const AVDictionaryEntry *prediction = av_dict_get(test->options, "pred-struct", NULL, 0);
	assert_non_null(prediction);
	assert_string_equal(prediction->value, "1");
	const AVDictionaryEntry *tune = av_dict_get(test->options, "tune", NULL, 0);
	assert_non_null(tune);
	assert_string_equal(tune->value, "0");
}

static void svt_option_failure(void **state)
{
	struct test_encoder *test = *state;
	set_options_error = AVERROR_OPTION_NOT_FOUND;
	assert_false(av1_update(&test->enc, test->settings));
	assert_int_equal(init_codec_calls, 0);
	assert_non_null(strstr(last_error, "SVT-AV1"));
	assert_non_null(strstr(last_error, av_err2str(set_options_error)));
	set_options_error = AVERROR(ENOMEM);
	last_error[0] = '\0';
	/* Also exercise the factory's cleanup of a partially initialized encoder. */
	assert_null(svt_av1_create(test->settings, NULL));
	assert_int_equal(init_codec_calls, 0);
	assert_non_null(strstr(last_error, "SVT-AV1"));
	assert_non_null(strstr(last_error, av_err2str(set_options_error)));
}

static void aom_cbr_unchanged(void **state)
{
	struct test_encoder *test = *state;
	const AVCodec *codec = avcodec_find_encoder_by_name("libaom-av1");
	if (!codec)
		skip();
	avcodec_free_context(&test->enc.ffve.context);
	test->enc.ffve.context = avcodec_alloc_context3(codec);
	assert_non_null(test->enc.ffve.context);
	test->enc.type = AV1_ENCODER_TYPE_AOM;
	set_options_error = AVERROR_OPTION_NOT_FOUND;
	assert_true(av1_update(&test->enc, test->settings));
	assert_int_equal(init_codec_calls, 1);
	assert_int_equal(test->enc.ffve.context->bit_rate, 6000000);
	assert_int_equal(test->enc.ffve.context->rc_min_rate, 6000000);
	assert_int_equal(test->enc.ffve.context->rc_max_rate, 6000000);
}

static void svt_encode_smoke(void **state)
{
	struct test_encoder *test = *state;
	check_rate_control(test, "CBR", "2", 6000000);
	AVCodecContext *context = test->enc.ffve.context;
	/* Validate the exact production dictionary; do not add test-only SVT options. */
	context->err_recognition = AV_EF_EXPLODE;
	assert_true(ffmpeg_video_encoder_init_codec(&test->enc.ffve));
	AVFrame *frame = test->enc.ffve.vframe;
	AVPacket packet = {0};
	int packets = 0;
	for (int i = 0; i < 8; i++) {
		assert_int_equal(av_frame_make_writable(frame), 0);
		for (int plane = 0; plane < 3; plane++) {
			const int size = plane ? 64 : 128;
			for (int row = 0; row < size; row++)
				memset(frame->data[plane] + row * frame->linesize[plane], plane ? 128 : 16 + i, size);
		}
		frame->pts = i;
		assert_int_equal(avcodec_send_frame(context, frame), 0);
		int ret;
		while ((ret = avcodec_receive_packet(context, &packet)) == 0) {
			packets++;
			av_packet_unref(&packet);
		}
		assert_int_equal(ret, AVERROR(EAGAIN));
	}
	assert_int_equal(avcodec_send_frame(context, NULL), 0);
	int ret;
	while ((ret = avcodec_receive_packet(context, &packet)) == 0) {
		packets++;
		av_packet_unref(&packet);
	}
	assert_int_equal(ret, AVERROR_EOF);
	assert_int_equal(packets, 8);
}

int main(void)
{
	if (!avcodec_find_encoder_by_name("libsvtav1")) {
		fprintf(stderr, "FFmpeg was built without libsvtav1; skipping SVT-AV1 tests.\n");
		return 77;
	}
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(svt_cbr, setup, teardown),
		cmocka_unit_test_setup_teardown(svt_vbr, setup, teardown),
		cmocka_unit_test_setup_teardown(svt_cqp, setup, teardown),
		cmocka_unit_test_setup_teardown(svt_custom_options, setup, teardown),
		cmocka_unit_test_setup_teardown(svt_custom_tuning_keeps_cbr, setup, teardown),
		cmocka_unit_test_setup_teardown(svt_option_failure, setup, teardown),
		cmocka_unit_test_setup_teardown(aom_cbr_unchanged, setup, teardown),
		cmocka_unit_test_setup_teardown(svt_encode_smoke, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
