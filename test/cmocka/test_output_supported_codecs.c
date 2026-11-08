/* Characterize borrowed codec metadata through the actual libobs exports. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void borrowed_codecs(void **state)
{
	(void)state;
	struct obs_output output = {0};
	struct obs_output before;
	const char video[] = {'h', '2', '6', '4', ';', (char)0xff, 0};
	const char audio[] = "aac;opus";
	const char *values[] = {NULL, "", video, audio};
	assert_null(obs_output_get_supported_video_codecs(NULL));
	assert_null(obs_output_get_supported_audio_codecs(NULL));
	for (size_t v = 0; v < 4; v++) {
		for (size_t a = 0; a < 4; a++) {
			output.info.encoded_video_codecs = values[v];
			output.info.encoded_audio_codecs = values[a];
			/* Metadata is returned even without encoded/video/audio flags. */
			output.info.flags = (uint32_t)(v * 4 + a);
			memcpy(&before, &output, sizeof(output));
			assert_ptr_equal(obs_output_get_supported_video_codecs(&output), values[v]);
			assert_ptr_equal(obs_output_get_supported_audio_codecs(&output), values[a]);
			assert_ptr_equal(obs_output_get_supported_video_codecs(&output), values[v]);
			assert_ptr_equal(obs_output_get_supported_audio_codecs(&output), values[a]);
			assert_memory_equal(&output, &before, sizeof(output));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_codecs)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
