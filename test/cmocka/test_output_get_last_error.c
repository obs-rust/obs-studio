/* Characterize the original C source and the loaded public API (#5294).
 * The future Rust swap can define OUTPUT_LAST_ERROR_EXTERNAL to run these
 * unchanged assertions solely against its public symbol. No fake objects. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#ifndef OUTPUT_LAST_ERROR_EXTERNAL
#ifndef OUTPUT_LAST_ERROR_SOURCE
#define OUTPUT_LAST_ERROR_SOURCE "libobs/obs-output-get-last-error.c.h"
#endif
#define obs_output_get_last_error output_last_error_original_c
#include OUTPUT_LAST_ERROR_SOURCE
#undef obs_output_get_last_error
#endif

static obs_output_t *output;
static obs_encoder_t *video[MAX_OUTPUT_VIDEO_ENCODERS];
static obs_encoder_t *audio[MAX_OUTPUT_AUDIO_ENCODERS];
static unsigned log_count;
static int log_level;
static char log_message[128];

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static const char *type_name(void *data)
{
	(void)data;
	return "last-error characterization";
}

static void *output_create(obs_data_t *settings, obs_output_t *owner)
{
	(void)settings;
	return owner;
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

static void *encoder_create(obs_data_t *settings, obs_encoder_t *owner)
{
	(void)settings;
	return owner;
}

static bool encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static size_t encoder_frame_size(void *data)
{
	(void)data;
	return 1024;
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_output_info info = {
		.id = "last_error_output",
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK | OBS_OUTPUT_MULTI_TRACK_VIDEO,
		.get_name = type_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&info);
	output = obs_output_create(info.id, "last-error output", NULL, NULL);
	assert_non_null(output);
	struct obs_encoder_info encoder_info = {
		.id = "last_error_video",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = type_name,
		.create = encoder_create,
		.destroy = output_destroy,
		.encode = encoder_encode,
	};
	obs_register_encoder(&encoder_info);
	encoder_info.id = "last_error_audio";
	encoder_info.type = OBS_ENCODER_AUDIO;
	encoder_info.codec = "aac";
	encoder_info.get_frame_size = encoder_frame_size;
	obs_register_encoder(&encoder_info);
	/* Real, inactive objects: no encoding or media setup is needed. */
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		video[i] = obs_video_encoder_create("last_error_video", "video", NULL, NULL);
		assert_non_null(video[i]);
	}
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		audio[i] = obs_audio_encoder_create("last_error_audio", "audio", NULL, i, NULL);
		assert_non_null(audio[i]);
	}
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_output_release(output);
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++)
		obs_encoder_release(video[i]);
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++)
		obs_encoder_release(audio[i]);
	obs_shutdown();
	return 0;
}

static void check_query(obs_output_t *owner, const char *expected)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	long allocations = bnum_allocs();
	const char *actual = obs_output_get_last_error(owner);
	unsigned count = log_count;
	int actual_level = log_level;
	char actual_message[sizeof(log_message)];
	memcpy(actual_message, log_message, sizeof(actual_message));
#ifndef OUTPUT_LAST_ERROR_EXTERNAL
	const char *original = output_last_error_original_c(owner);
#endif
	base_set_log_handler(old_handler, old_param);
	assert_ptr_equal(actual, expected);
#ifndef OUTPUT_LAST_ERROR_EXTERNAL
	assert_ptr_equal(original, expected);
	assert_int_equal(log_count, 2 * count);
#endif
	assert_int_equal(bnum_allocs(), allocations);
	assert_int_equal(count, owner ? 0 : 1);
	if (!owner) {
		assert_int_equal(actual_level, LOG_DEBUG);
		assert_string_equal(actual_message, "obs_output_get_last_error: Null 'output' parameter");
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(log_message, "obs_output_get_last_error: Null 'output' parameter");
	} else if (expected) {
		assert_string_equal(actual, expected);
	}
}

static void null_diagnostic(void **state)
{
	(void)state;
	check_query(NULL, NULL);
}

static void fallback_order_and_borrowing(void **state)
{
	(void)state;
	check_query(output, NULL);
	/* Attach every slot through the real ownership APIs. NULL errors on
	 * present encoders must be skipped just like empty encoder slots. */
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		obs_output_set_video_encoder2(output, video[i], i);
		assert_ptr_equal(output->video_encoders[i], video[i]);
	}
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		obs_output_set_audio_encoder(output, audio[i], i);
		assert_ptr_equal(output->audio_encoders[i], audio[i]);
	}
	check_query(output, NULL);
	/* Equal bytes in independent allocations expose copying and returning
	 * the wrong encoder even when a string-only assertion would pass. */
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++)
		obs_encoder_set_last_error(audio[i], "same error");
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++)
		obs_encoder_set_last_error(video[i], "same error");
	obs_output_set_last_error(output, "same error");
	check_query(output, output->last_error_message);
	/* Characterized, not endorsed: empty strings are errors, not fallback. */
	obs_output_set_last_error(output, "");
	check_query(output, output->last_error_message);
	obs_output_set_last_error(output, NULL);
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		check_query(output, video[i]->last_error_message);
		obs_encoder_set_last_error(video[i], "");
		check_query(output, video[i]->last_error_message);
		obs_encoder_set_last_error(video[i], NULL);
		check_query(output, i + 1 < MAX_OUTPUT_VIDEO_ENCODERS ? video[i + 1]->last_error_message
								      : audio[0]->last_error_message);
		obs_output_set_video_encoder2(output, NULL, i);
	}
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		check_query(output, audio[i]->last_error_message);
		obs_encoder_set_last_error(audio[i], "");
		check_query(output, audio[i]->last_error_message);
		obs_encoder_set_last_error(audio[i], NULL);
		check_query(output, i + 1 < MAX_OUTPUT_AUDIO_ENCODERS ? audio[i + 1]->last_error_message : NULL);
		obs_output_set_audio_encoder(output, NULL, i);
	}
	check_query(output, NULL);
	/* Last-slot errors with holes before them, including non-UTF-8 bytes. */
	const char bytes[] = {(char)0xff, (char)0x80, 'x', 0};
	obs_encoder_t *last_video = video[MAX_OUTPUT_VIDEO_ENCODERS - 1];
	obs_encoder_t *last_audio = audio[MAX_OUTPUT_AUDIO_ENCODERS - 1];
	obs_output_set_video_encoder2(output, last_video, MAX_OUTPUT_VIDEO_ENCODERS - 1);
	obs_output_set_audio_encoder(output, last_audio, MAX_OUTPUT_AUDIO_ENCODERS - 1);
	obs_encoder_set_last_error(last_video, bytes);
	obs_encoder_set_last_error(last_audio, bytes);
	/* A borrowed query must not acquire the encoder error mutexes. Holding
	 * both on this thread makes an added lock fail the CTest timeout. */
	assert_int_equal(pthread_mutex_lock(&last_video->last_error_mutex), 0);
	assert_int_equal(pthread_mutex_lock(&last_audio->last_error_mutex), 0);
	check_query(output, last_video->last_error_message);
	assert_int_equal(pthread_mutex_unlock(&last_audio->last_error_mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&last_video->last_error_mutex), 0);
	obs_output_set_video_encoder2(output, NULL, MAX_OUTPUT_VIDEO_ENCODERS - 1);
	check_query(output, last_audio->last_error_message);
	obs_output_set_audio_encoder(output, NULL, MAX_OUTPUT_AUDIO_ENCODERS - 1);
	check_query(output, NULL);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_diagnostic),
					   cmocka_unit_test(fallback_order_and_borrowing)};
	return cmocka_run_group_tests(tests, setup, teardown);
}
