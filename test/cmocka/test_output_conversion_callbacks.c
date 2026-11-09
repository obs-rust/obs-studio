/* Characterize the actual libobs setters and packet callback registration.
 * Stack fixtures isolate these operations without starting media or OBS. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[256];
static log_handler_t saved_handler;
static void *saved_param;

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

static int setup(void **state)
{
	(void)state;
	log_count = 0;
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(saved_handler, saved_param);
	return 0;
}

static void expect_log(int level, const char *message)
{
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, level);
	assert_string_equal(log_message, message);
	log_count = 0;
}

static void conversion_guards(void **state)
{
	(void)state;
	struct obs_output output = {0};
	output.context.name = "fixture";
	struct video_scale_info video = {.format = VIDEO_FORMAT_I420, .width = 320, .height = 240};
	struct audio_convert_info audio = {.format = AUDIO_FORMAT_FLOAT,
					   .samples_per_sec = 48000,
					   .speakers = SPEAKERS_STEREO};
	/* Both null: the output guard wins over the conversion guard. */
	obs_output_set_video_conversion(NULL, NULL);
	expect_log(LOG_DEBUG, "obs_output_set_video_conversion: Null 'output' parameter");
	obs_output_set_audio_conversion(NULL, NULL);
	expect_log(LOG_DEBUG, "obs_output_set_audio_conversion: Null 'output' parameter");
	obs_output_set_video_conversion(NULL, &video);
	expect_log(LOG_DEBUG, "obs_output_set_video_conversion: Null 'output' parameter");
	obs_output_set_audio_conversion(NULL, &audio);
	expect_log(LOG_DEBUG, "obs_output_set_audio_conversion: Null 'output' parameter");

	const uint32_t flags[] = {0,
				  OBS_OUTPUT_VIDEO,
				  OBS_OUTPUT_AUDIO,
				  OBS_OUTPUT_AV,
				  OBS_OUTPUT_ENCODED,
				  OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED};
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++) {
		output.info.flags = flags[i];
		/* Pin unchanged prior state, including a previously set conversion. */
		output.video_conversion = video;
		output.audio_conversion = audio;
		output.video_conversion_set = true;
		output.audio_conversion_set = true;
		struct obs_output before;
		memcpy(&before, &output, sizeof(before));
		obs_output_set_video_conversion(&output, NULL);
		expect_log(LOG_DEBUG, "obs_output_set_video_conversion: Null 'conversion' parameter");
		obs_output_set_audio_conversion(&output, NULL);
		expect_log(LOG_DEBUG, "obs_output_set_audio_conversion: Null 'conversion' parameter");
		assert_memory_equal(&output, &before, sizeof(output));
		if ((flags[i] & OBS_OUTPUT_ENCODED) || !(flags[i] & OBS_OUTPUT_VIDEO)) {
			obs_output_set_video_conversion(&output, &video);
			expect_log(
				LOG_WARNING,
				(flags[i] & OBS_OUTPUT_ENCODED)
					? "Output 'fixture': Tried to use obs_output_set_video_conversion on an encoded output"
					: "Output 'fixture': Tried to use obs_output_set_video_conversion on a non-video output");
			assert_memory_equal(&output, &before, sizeof(output));
		}
		if ((flags[i] & OBS_OUTPUT_ENCODED) || !(flags[i] & OBS_OUTPUT_AUDIO)) {
			obs_output_set_audio_conversion(&output, &audio);
			expect_log(
				LOG_WARNING,
				(flags[i] & OBS_OUTPUT_ENCODED)
					? "Output 'fixture': Tried to use obs_output_set_audio_conversion on an encoded output"
					: "Output 'fixture': Tried to use obs_output_set_audio_conversion on a non-audio output");
			assert_memory_equal(&output, &before, sizeof(output));
		}
	}
}

static void conversion_copies(void **state)
{
	(void)state;
	const uint32_t flags[] = {OBS_OUTPUT_VIDEO, OBS_OUTPUT_AUDIO, OBS_OUTPUT_AV};
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++) {
		struct obs_output output = {0};
		output.info.flags = flags[i];
		struct video_scale_info video = {.format = VIDEO_FORMAT_NV12,
						 .width = 641,
						 .height = 359,
						 .colorspace = VIDEO_CS_709,
						 .range = VIDEO_RANGE_FULL};
		struct audio_convert_info audio = {.format = AUDIO_FORMAT_FLOAT_PLANAR,
						   .samples_per_sec = 44100,
						   .speakers = SPEAKERS_5POINT1};
		if (flags[i] & OBS_OUTPUT_VIDEO) {
			obs_output_set_video_conversion(&output, &video);
			assert_true(output.video_conversion_set);
			assert_memory_equal(&output.video_conversion, &video, sizeof(video));
			video.width = 0;
			assert_int_equal(output.video_conversion.width, 641);
			/* Zero sizes are stored verbatim, characterized, not endorsed. */
			video.height = 0;
			obs_output_set_video_conversion(&output, &video);
			assert_memory_equal(&output.video_conversion, &video, sizeof(video));
		}
		if (flags[i] & OBS_OUTPUT_AUDIO) {
			obs_output_set_audio_conversion(&output, &audio);
			assert_true(output.audio_conversion_set);
			assert_memory_equal(&output.audio_conversion, &audio, sizeof(audio));
			audio.samples_per_sec = 0;
			assert_int_equal(output.audio_conversion.samples_per_sec, 44100);
			/* No sample-rate validation here, characterized, not endorsed. */
			obs_output_set_audio_conversion(&output, &audio);
			assert_memory_equal(&output.audio_conversion, &audio, sizeof(audio));
		}
	}
	assert_int_equal(log_count, 0);
}

static void packet_a(obs_output_t *output, struct encoder_packet *packet, struct encoder_packet_time *time, void *param)
{
	(void)output;
	(void)packet;
	(void)time;
	(void)param;
	fail(); /* Registration must never invoke a callback. */
}

static void packet_b(obs_output_t *output, struct encoder_packet *packet, struct encoder_packet_time *time, void *param)
{
	packet_a(output, packet, time, param);
}

static void packet_callbacks(void **state)
{
	(void)state;
	struct obs_output output = {0};
	int first = 1, second = 2;
	assert_int_equal(pthread_mutex_init(&output.pkt_callbacks_mutex, NULL), 0);
	obs_output_remove_packet_callback(&output, packet_a, &first);
	assert_int_equal(output.pkt_callbacks.num, 0);
	obs_output_add_packet_callback(&output, packet_a, &first);
	obs_output_add_packet_callback(&output, packet_b, &first);
	obs_output_add_packet_callback(&output, packet_a, &second);
	obs_output_add_packet_callback(&output, packet_a, &first);
	assert_int_equal(output.pkt_callbacks.num, 4);
	assert_true(output.pkt_callbacks.array[0].packet_cb == packet_a);
	assert_ptr_equal(output.pkt_callbacks.array[0].param, &first);
	assert_ptr_equal(output.pkt_callbacks.array[1].param, &second);
	assert_true(output.pkt_callbacks.array[2].packet_cb == packet_b);
	assert_ptr_equal(output.pkt_callbacks.array[3].param, &first);
	/* Duplicates are allowed; remove erases only the newest exact pair. */
	obs_output_remove_packet_callback(&output, packet_a, &first);
	assert_int_equal(output.pkt_callbacks.num, 3);
	assert_ptr_equal(output.pkt_callbacks.array[0].param, &second);
	assert_true(output.pkt_callbacks.array[1].packet_cb == packet_b);
	assert_ptr_equal(output.pkt_callbacks.array[2].param, &first);
	obs_output_remove_packet_callback(&output, packet_b, &second);
	assert_int_equal(output.pkt_callbacks.num, 3);
	obs_output_remove_packet_callback(&output, packet_a, &second);
	obs_output_remove_packet_callback(&output, packet_a, &first);
	assert_int_equal(output.pkt_callbacks.num, 1);
	assert_true(output.pkt_callbacks.array[0].packet_cb == packet_b);
	obs_output_remove_packet_callback(&output, packet_b, &first);
	/* NULL callback and parameter are stored too, characterized, not endorsed. */
	obs_output_add_packet_callback(&output, NULL, NULL);
	assert_int_equal(output.pkt_callbacks.num, 1);
	assert_true(output.pkt_callbacks.array[0].packet_cb == NULL);
	assert_null(output.pkt_callbacks.array[0].param);
	obs_output_remove_packet_callback(&output, NULL, NULL);
	assert_int_equal(output.pkt_callbacks.num, 0);
	da_free(output.pkt_callbacks);
	assert_int_equal(pthread_mutex_destroy(&output.pkt_callbacks_mutex), 0);
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(conversion_guards, setup, teardown),
					   cmocka_unit_test_setup_teardown(conversion_copies, setup, teardown),
					   cmocka_unit_test_setup_teardown(packet_callbacks, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
