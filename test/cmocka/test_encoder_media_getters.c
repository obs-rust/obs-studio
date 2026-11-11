/* Characterize the public media getters through each real libobs library.
 * The opaque media tokens are borrowed identities and are never dereferenced. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

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

static int setup(void **state)
{
	(void)state;
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
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

static void null_encoders(void **state)
{
	(void)state;
	assert_null(obs_encoder_video(NULL));
	expect_log(LOG_DEBUG, "obs_encoder_video: Null 'encoder' parameter");
	assert_null(obs_encoder_parent_video(NULL));
	expect_log(LOG_DEBUG, "obs_encoder_parent_video: Null 'encoder' parameter");
	assert_null(obs_encoder_audio(NULL));
	expect_log(LOG_DEBUG, "obs_encoder_audio: Null 'encoder' parameter");
}

static void video_getters(void **state)
{
	(void)state;
	uint64_t parent_token = 0, override_token = 0;
	char name[] = "media-%s";
	video_t *(*getters[])(const obs_encoder_t *) = {obs_encoder_video, obs_encoder_parent_video};
	/* Historical setter label in obs_encoder_video is characterized, not endorsed. */
	const char *warnings[] = {"obs_encoder_set_video: encoder 'media-%s' is not a video encoder",
				  "obs_encoder_parent_video: encoder 'media-%s' is not a video encoder"};
	const enum obs_encoder_type types[] = {OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO, (enum obs_encoder_type)2};
	for (size_t getter = 0; getter < 2; getter++) {
		for (size_t type = 0; type < 3; type++) {
			for (int media = 0; media < 2; media++) {
				for (int override = 0; override < 2; override++) {
					for (int started = 0; started < 2; started++) {
						struct obs_encoder encoder = {0}, before;
						encoder.context.name = name;
						encoder.info.type = types[type];
						encoder.orig_info.type = types[type] == OBS_ENCODER_VIDEO
										 ? OBS_ENCODER_AUDIO
										 : OBS_ENCODER_VIDEO;
						encoder.media = media ? &parent_token : NULL;
						encoder.fps_override = override ? (video_t *)&override_token : NULL;
						encoder.active = started;
						encoder.initialized = started;
						memcpy(&before, &encoder, sizeof(before));
						video_t *expected = encoder.media;
						if (getter == 0 && override)
							expected = encoder.fps_override;
						if (types[type] != OBS_ENCODER_VIDEO)
							expected = NULL;
						assert_ptr_equal(getters[getter](&encoder), expected);
						if (types[type] != OBS_ENCODER_VIDEO)
							expect_log(LOG_WARNING, warnings[getter]);
						assert_int_equal(log_count, 0);
						assert_memory_equal(&encoder, &before, sizeof(encoder));
					}
				}
			}
		}
	}
}

static void audio_getter(void **state)
{
	(void)state;
	uint64_t media_token = 0, override_token = 0;
	char name[] = "media-%s";
	const enum obs_encoder_type types[] = {OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO, (enum obs_encoder_type)2};
	for (size_t type = 0; type < 3; type++) {
		for (int media = 0; media < 2; media++) {
			for (int override = 0; override < 2; override++) {
				for (int started = 0; started < 2; started++) {
					struct obs_encoder encoder = {0}, before;
					encoder.context.name = name;
					encoder.info.type = types[type];
					encoder.orig_info.type = types[type] == OBS_ENCODER_AUDIO ? OBS_ENCODER_VIDEO
												  : OBS_ENCODER_AUDIO;
					encoder.media = media ? &media_token : NULL;
					encoder.fps_override = override ? (video_t *)&override_token : NULL;
					encoder.active = started;
					encoder.initialized = started;
					memcpy(&before, &encoder, sizeof(before));
					audio_t *expected = types[type] == OBS_ENCODER_AUDIO ? encoder.media : NULL;
					assert_ptr_equal(obs_encoder_audio(&encoder), expected);
					/* Historical setter label is characterized, not endorsed. */
					if (types[type] != OBS_ENCODER_AUDIO)
						expect_log(
							LOG_WARNING,
							"obs_encoder_set_audio: encoder 'media-%s' is not an audio encoder");
					assert_int_equal(log_count, 0);
					assert_memory_equal(&encoder, &before, sizeof(encoder));
				}
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(null_encoders, setup, teardown),
					   cmocka_unit_test_setup_teardown(video_getters, setup, teardown),
					   cmocka_unit_test_setup_teardown(audio_getter, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
