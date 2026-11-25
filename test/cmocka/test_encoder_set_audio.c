/* Actual-library characterization of obs_encoder_set_audio. The encoder is
 * caller-owned private state; the media objects are real audio outputs. No
 * copied setter or stand-in audio layout participates in these assertions. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static audio_t *media[3];
static log_handler_t saved_handler;
static void *saved_param;
static unsigned log_count;
static int log_level;
static char log_message[256];

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

static bool input(void *param, uint64_t start, uint64_t end, uint64_t *timestamp, uint32_t mixers,
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

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	const uint32_t rates[] = {48000, 44100};
	for (size_t i = 0; i < 2; i++) {
		struct audio_output_info info = {.name = "set-audio",
						 .samples_per_sec = rates[i],
						 .format = AUDIO_FORMAT_FLOAT_PLANAR,
						 .speakers = SPEAKERS_STEREO,
						 .input_callback = input};
		assert_int_equal(audio_output_open(&media[i + 1], &info), AUDIO_OUTPUT_SUCCESS);
	}
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(saved_handler, saved_param);
	audio_output_close(media[1]);
	audio_output_close(media[2]);
	obs_shutdown();
	return 0;
}

static void null_encoder(void **state)
{
	(void)state;
	for (size_t i = 0; i < 3; i++) {
		log_count = 0;
		obs_encoder_set_audio(NULL, media[i]);
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(log_message, "obs_encoder_set_audio: Null 'encoder' parameter");
	}
}

static void binding_matrix(void **state)
{
	(void)state;
	const enum obs_encoder_type types[] = {OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO};
	const uint32_t rates[] = {0, 48000, 44100};
	for (size_t type = 0; type < 2; type++) {
		for (unsigned active = 0; active < 2; active++) {
			for (unsigned initialized = 0; initialized < 2; initialized++) {
				for (size_t old = 0; old < 3; old++) {
					for (size_t next = 0; next < 3; next++) {
						struct obs_encoder encoder, expected;
						memset(&encoder, 0, sizeof(encoder));
						encoder.context.name = "audio-%s";
						encoder.info.type = types[type];
						/* The current type, not orig_info.type, decides admission. */
						encoder.orig_info.type = types[1 - type];
						os_atomic_set_bool(&encoder.active, active != 0);
						encoder.initialized = initialized != 0;
						encoder.media = media[old];
						encoder.timebase_num = 7;
						encoder.timebase_den = 90000;
						encoder.samplerate = 32000;
						encoder.mixer_idx = 3;
						memcpy(&expected, &encoder, sizeof(expected));
						log_count = 0;
						obs_encoder_set_audio(&encoder, media[next]);
						if (type != 0) {
							/* Characterized, not endorsed: wrong type wins even when active. */
							assert_int_equal(log_count, 1);
							assert_int_equal(log_level, LOG_WARNING);
							assert_string_equal(
								log_message,
								"obs_encoder_set_audio: encoder 'audio-%s' is not an audio encoder");
						} else if (active) {
							/* Same-media assignment and detach are both refused. */
							assert_int_equal(log_count, 1);
							assert_int_equal(log_level, LOG_WARNING);
							assert_string_equal(
								log_message,
								"encoder 'audio-%s': Cannot apply a new audio_t object while the encoder is active");
						} else {
							assert_int_equal(log_count, 0);
							expected.media = media[next];
							/* Rebinding the same object still refreshes both timebase fields;
							 * initialized and samplerate do not override the media's rate. */
							expected.timebase_num = next ? 1 : 0;
							expected.timebase_den = rates[next];
						}
						assert_memory_equal(&encoder, &expected, sizeof(encoder));
					}
				}
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_encoder), cmocka_unit_test(binding_matrix)};
	return cmocka_run_group_tests(tests, setup, teardown);
}
