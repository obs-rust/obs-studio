/* Actual-library characterization, kept unchanged through the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct media_log {
	unsigned calls;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_media_log(int level, const char *format, va_list args, void *param)
{
	struct media_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void supported_media(void **state)
{
	(void)state;
	/* Only pointer identity is used: these tokens are never dereferenced. */
	int tokens[4];
	const uint32_t flags[] = {0, OBS_OUTPUT_VIDEO, OBS_OUTPUT_AUDIO, OBS_OUTPUT_AV};
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++) {
		for (unsigned encoded = 0; encoded < 2; encoded++) {
			struct obs_output output = {0};
			output.context.name = "media-test";
			output.info.flags = flags[i] | (encoded ? OBS_OUTPUT_ENCODED : 0);
			output.video = (video_t *)&tokens[0];
			output.audio = (audio_t *)&tokens[1];
			for (unsigned clear = 0; clear < 2; clear++) {
				video_t *video = clear ? NULL : (video_t *)&tokens[2];
				audio_t *audio = clear ? NULL : (audio_t *)&tokens[3];
				unsigned char expected[sizeof(output)];
				memcpy(expected, &output, sizeof(output));
				if (!encoded && (flags[i] & OBS_OUTPUT_VIDEO))
					memcpy(expected + offsetof(struct obs_output, video), &video, sizeof(video));
				if (!encoded && (flags[i] & OBS_OUTPUT_AUDIO))
					memcpy(expected + offsetof(struct obs_output, audio), &audio, sizeof(audio));
				struct media_log log = {0};
				log_handler_t old;
				void *param;
				base_get_log_handler(&old, &param);
				base_set_log_handler(capture_media_log, &log);
				obs_output_set_media(&output, video, audio);
				base_set_log_handler(old, param);
				assert_memory_equal(&output, expected, sizeof(output));
				assert_int_equal(log.calls, encoded);
				if (encoded) {
					assert_int_equal(log.level, LOG_WARNING);
					assert_string_equal(
						log.message,
						"Output 'media-test': Tried to use obs_output_set_media on an encoded output");
				}
			}
		}
	}
}

static void null_output(void **state)
{
	(void)state;
	struct media_log log = {0};
	log_handler_t old;
	void *param;
	base_get_log_handler(&old, &param);
	base_set_log_handler(capture_media_log, &log);
	obs_output_set_media(NULL, NULL, NULL);
	base_set_log_handler(old, param);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_output_set_media: Null 'output' parameter");
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(supported_media), cmocka_unit_test(null_output)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
