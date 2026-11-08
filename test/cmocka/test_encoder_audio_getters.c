/* Actual-library characterization: public audio getters. */
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
static size_t sample_rate(const obs_encoder_t *encoder)
{
	return obs_encoder_get_sample_rate(encoder);
}
static void characterize(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	audio_t *audio = NULL;
	struct audio_output_info info = {.name = "audio-getters",
					 .samples_per_sec = 48000,
					 .format = AUDIO_FORMAT_FLOAT_PLANAR,
					 .speakers = SPEAKERS_STEREO,
					 .input_callback = input};
	assert_int_equal(audio_output_open(&audio, &info), AUDIO_OUTPUT_SUCCESS);
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	size_t (*getters[])(const obs_encoder_t *) = {sample_rate, obs_encoder_get_frame_size,
						      obs_encoder_get_mixer_index};
	const char *names[] = {"obs_encoder_get_sample_rate", "obs_encoder_get_frame_size",
			       "obs_encoder_get_mixer_index"};
	const size_t values[] = {0, 1, 1024, UINT32_MAX, SIZE_MAX};
	for (size_t getter = 0; getter < 3; getter++) {
		log_count = 0;
		assert_int_equal(getters[getter](NULL), 0);
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		char expected[256];
		snprintf(expected, sizeof(expected), "%s: Null 'encoder' parameter", names[getter]);
		assert_string_equal(log_message, expected);
		for (int type = OBS_ENCODER_AUDIO; type <= OBS_ENCODER_VIDEO; type++) {
			for (int media = 0; media < 2; media++) {
				for (size_t x = 0; x < 5; x++) {
					for (size_t y = 0; y < 5; y++) {
						struct obs_encoder encoder = {0}, before;
						encoder.context.name = "audio-%s";
						encoder.info.type = (enum obs_encoder_type)type;
						encoder.orig_info.type = type == OBS_ENCODER_AUDIO ? OBS_ENCODER_VIDEO
												   : OBS_ENCODER_AUDIO;
						encoder.media = media ? audio : NULL;
						encoder.samplerate = (uint32_t)values[x];
						encoder.framesize = values[x];
						encoder.mixer_idx = values[y];
						memcpy(&before, &encoder, sizeof(before));
						size_t want = getter == 1 ? values[x] : values[y];
						/* Characterized, not endorsed: no media hides the stored sample rate. */
						if (getter == 0)
							want = media ? (encoder.samplerate ? encoder.samplerate : 48000)
								     : 0;
						if (type != OBS_ENCODER_AUDIO)
							want = 0;
						log_count = 0;
						assert_int_equal(getters[getter](&encoder), want);
						assert_int_equal(log_count, type == OBS_ENCODER_AUDIO ? 0 : 1);
						if (type != OBS_ENCODER_AUDIO) {
							assert_int_equal(log_level, LOG_WARNING);
							snprintf(expected, sizeof(expected),
								 "%s: encoder 'audio-%%s' is not an audio encoder",
								 names[getter]);
							assert_string_equal(log_message, expected);
						}
						assert_memory_equal(&encoder, &before, sizeof(encoder));
					}
				}
			}
		}
	}
	base_set_log_handler(old_handler, old_param);
	audio_output_close(audio);
	obs_shutdown();
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(characterize)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
