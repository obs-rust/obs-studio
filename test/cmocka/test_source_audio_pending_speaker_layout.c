/* Characterization of obs_source_audio_pending() and
 * obs_source_get_speaker_layout() (#5603), the two audio getters scenes,
 * transitions, the stinger, the slideshows, the noise filters and the volume
 * meter read every tick.
 *
 * What it pins, on a test-owned struct obs_source (no core needed):
 *
 *   - NULL: audio_pending answers true and logs one LOG_DEBUG line,
 *     "obs_source_audio_pending: Null 'source' parameter";
 *     get_speaker_layout answers SPEAKERS_UNKNOWN and logs
 *     "obs_source_get_audio_channels: Null 'source' parameter" (the name
 *     upstream passes to its validity check, kept);
 *   - a removed source answers pending, whatever its flags and audio_pending;
 *   - a source whose info.output_flags has neither OBS_SOURCE_AUDIO nor
 *     OBS_SOURCE_COMPOSITE answers pending;
 *   - an audio or composite source answers its audio_pending field;
 *   - get_speaker_layout answers sample_info.speakers, removed or not;
 *   - a non-NULL source is never logged about, and neither call writes to it. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

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

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

static const unsigned char fills[] = {0x00, 0xff, 0x5a};

/* Flag words over the two bits that matter and their neighbours: none, each
 * alone, both, every other bit, and all bits. */
static const uint32_t flag_words[] = {
	0,
	OBS_SOURCE_VIDEO,
	OBS_SOURCE_ASYNC_VIDEO,
	OBS_SOURCE_AUDIO,
	OBS_SOURCE_COMPOSITE,
	OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE,
	OBS_SOURCE_AUDIO | OBS_SOURCE_VIDEO,
	OBS_SOURCE_COMPOSITE | OBS_SOURCE_VIDEO,
	~(uint32_t)(OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE),
	~(uint32_t)OBS_SOURCE_AUDIO,
	~(uint32_t)OBS_SOURCE_COMPOSITE,
	UINT32_MAX,
};

static void null_source_answers_the_defaults_and_names_the_caller(void **state)
{
	(void)state;
	capture_start();
	bool pending = obs_source_audio_pending(NULL);
	capture_stop();
	assert_true(pending);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_audio_pending: Null 'source' parameter");

	capture_start();
	enum speaker_layout layout = obs_source_get_speaker_layout(NULL);
	capture_stop();
	assert_int_equal(layout, SPEAKERS_UNKNOWN);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	/* Upstream's validity check names obs_source_get_audio_channels. */
	assert_string_equal(log_message, "obs_source_get_audio_channels: Null 'source' parameter");
}

static void audio_pending_follows_removed_flags_and_field(void **state)
{
	(void)state;
	struct obs_source source;
	unsigned char snapshot[sizeof(source)];

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (size_t w = 0; w < sizeof(flag_words) / sizeof(flag_words[0]); w++) {
			for (unsigned removed = 0; removed < 2; removed++) {
				for (unsigned field = 0; field < 2; field++) {
					/* Every other byte set to the fill, so a getter
					 * reading any other field can't pass by finding
					 * the expected answer there. */
					memset(&source, fills[f], sizeof(source));
					source.info.output_flags = flag_words[w];
					source.removed = removed != 0;
					source.audio_pending = field != 0;
					memcpy(snapshot, &source, sizeof(source));

					const uint32_t flags = flag_words[w];
					const bool reads_field = (flags & (OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE)) !=
								 0;
					const bool expected = removed || !reads_field || field;

					for (unsigned repeat = 0; repeat < 2; repeat++) {
						assert_int_equal(obs_source_audio_pending(&source), expected);
						assert_memory_equal(snapshot, &source, sizeof(source));
					}
				}
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void audio_pending_named_cases(void **state)
{
	(void)state;
	struct obs_source source;

	capture_start();
	memset(&source, 0, sizeof(source));

	/* A removed audio source with nothing pending still answers pending. */
	source.info.output_flags = OBS_SOURCE_AUDIO;
	source.removed = true;
	source.audio_pending = false;
	assert_true(obs_source_audio_pending(&source));

	/* A video-only source has no audio to wait for: always pending. */
	source.removed = false;
	source.info.output_flags = OBS_SOURCE_VIDEO;
	assert_true(obs_source_audio_pending(&source));

	/* Audio and composite sources answer their field. */
	source.info.output_flags = OBS_SOURCE_AUDIO;
	assert_false(obs_source_audio_pending(&source));
	source.audio_pending = true;
	assert_true(obs_source_audio_pending(&source));

	source.info.output_flags = OBS_SOURCE_COMPOSITE;
	source.audio_pending = false;
	assert_false(obs_source_audio_pending(&source));
	source.audio_pending = true;
	assert_true(obs_source_audio_pending(&source));
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void speaker_layout_answers_sample_info_speakers(void **state)
{
	(void)state;
	static const int layouts[] = {
		SPEAKERS_UNKNOWN,
		SPEAKERS_MONO,
		SPEAKERS_STEREO,
		SPEAKERS_2POINT1,
		SPEAKERS_4POINT0,
		SPEAKERS_4POINT1,
		SPEAKERS_5POINT1,
		SPEAKERS_7POINT1,
		7,
		0x5a,
	};
	struct obs_source source;
	unsigned char snapshot[sizeof(source)];

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (size_t l = 0; l < sizeof(layouts) / sizeof(layouts[0]); l++) {
			for (unsigned removed = 0; removed < 2; removed++) {
				memset(&source, fills[f], sizeof(source));
				source.sample_info.speakers = (enum speaker_layout)layouts[l];
				source.removed = removed != 0;
				source.audio_pending = removed == 0;
				memcpy(snapshot, &source, sizeof(source));
				for (unsigned repeat = 0; repeat < 2; repeat++) {
					assert_int_equal(obs_source_get_speaker_layout(&source), layouts[l]);
					assert_memory_equal(snapshot, &source, sizeof(source));
				}
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source_answers_the_defaults_and_names_the_caller),
		cmocka_unit_test(audio_pending_follows_removed_flags_and_field),
		cmocka_unit_test(audio_pending_named_cases),
		cmocka_unit_test(speaker_layout_answers_sample_info_speakers),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
