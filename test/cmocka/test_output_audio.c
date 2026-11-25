/* Direct actual-library characterization of obs_output_audio (Refs #5655).
 * The output and encoder structs borrow real audio handles; this read-only API
 * needs no registered plugin, mutexes, or encoder initialization.
 * Keep unchanged through the later Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static audio_t *media[2];

struct captured_log {
	unsigned calls;
	int level;
	char message[192];
};

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct captured_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static void null_output(void **state)
{
	(void)state;
	log_handler_t handler;
	void *param;
	struct captured_log log = {0};
	base_get_log_handler(&handler, &param);
	base_set_log_handler(capture_log, &log);
	audio_t *result = obs_output_audio(NULL);
	base_set_log_handler(handler, param);
	assert_null(result);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_output_audio: Null 'output' parameter");
}

static void check_audio(const struct obs_output *output, audio_t *expected)
{
	unsigned char before[sizeof(*output)];
	memcpy(before, output, sizeof(*output));
	assert_ptr_equal(obs_output_audio(output), expected);
	assert_memory_equal(output, before, sizeof(*output));
}

static void raw_uses_own_audio(void **state)
{
	(void)state;
	struct obs_output output = {0};
	struct obs_encoder encoder = {0};
	encoder.info.type = OBS_ENCODER_AUDIO;
	encoder.media = media[1];
	output.audio_encoders[0] = &encoder;
	/* The AUDIO flag does not gate this getter: characterized, not endorsed. */
	const uint32_t flags[] = {0, OBS_OUTPUT_AUDIO, OBS_OUTPUT_VIDEO, OBS_OUTPUT_AV | OBS_OUTPUT_MULTI_TRACK};
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++) {
		output.info.flags = flags[i];
		output.audio = media[0];
		check_audio(&output, media[0]);
		output.audio = NULL;
		check_audio(&output, NULL);
	}
}

static void encoded_uses_first_occupied_slot(void **state)
{
	(void)state;
	struct obs_output output = {0};
	struct obs_encoder encoders[MAX_OUTPUT_AUDIO_ENCODERS] = {0};
	unsigned char before[sizeof(encoders)];
	output.audio = media[0];
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		encoders[i].info.type = OBS_ENCODER_AUDIO;
		encoders[i].media = media[i % 2];
	}
	memcpy(before, encoders, sizeof(encoders));
	/* Every occupancy mask includes gaps, slot zero, and the last slot.
	 * MULTI_TRACK and AUDIO flags do not restrict this read-only lookup. */
	const uint32_t flags[] = {OBS_OUTPUT_ENCODED, OBS_OUTPUT_ENCODED | OBS_OUTPUT_AUDIO,
				  OBS_OUTPUT_ENCODED | OBS_OUTPUT_AV | OBS_OUTPUT_MULTI_TRACK};
	for (size_t f = 0; f < sizeof(flags) / sizeof(flags[0]); f++) {
		output.info.flags = flags[f];
		for (uint32_t mask = 0; mask < (1U << MAX_OUTPUT_AUDIO_ENCODERS); mask++) {
			audio_t *expected = NULL;
			bool found = false;
			for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
				output.audio_encoders[i] = (mask & (1U << i)) ? &encoders[i] : NULL;
				if (!found && output.audio_encoders[i]) {
					expected = encoders[i].media;
					found = true;
				}
			}
			check_audio(&output, expected);
			assert_memory_equal(encoders, before, sizeof(encoders));
		}
	}
}

static void first_encoder_without_media_does_not_fall_through(void **state)
{
	(void)state;
	struct obs_output output = {0};
	struct obs_encoder empty = {0}, later = {0};
	empty.info.type = later.info.type = OBS_ENCODER_AUDIO;
	later.media = media[1];
	output.info.flags = OBS_OUTPUT_AUDIO | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK;
	output.audio = media[0];
	output.audio_encoders[MAX_OUTPUT_AUDIO_ENCODERS - 1] = &later;
	for (size_t i = 0; i + 1 < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		output.audio_encoders[i] = &empty;
		check_audio(&output, NULL);
		output.audio_encoders[i] = NULL;
		check_audio(&output, media[1]);
	}
}

static bool input_audio(void *param, uint64_t start, uint64_t end, uint64_t *timestamp, uint32_t mixers,
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
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct audio_output_info info = {
		.name = "output audio characterization",
		.samples_per_sec = 48000,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.speakers = SPEAKERS_STEREO,
		.input_callback = input_audio,
	};
	for (size_t i = 0; i < 2; i++) {
		if (audio_output_open(&media[i], &info) != AUDIO_OUTPUT_SUCCESS) {
			for (size_t j = 0; j < i; j++)
				audio_output_close(media[j]);
			obs_shutdown();
			return -1;
		}
	}
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	for (size_t i = 0; i < 2; i++)
		audio_output_close(media[i]);
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_output),
		cmocka_unit_test(raw_uses_own_audio),
		cmocka_unit_test(encoded_uses_first_occupied_slot),
		cmocka_unit_test(first_encoder_without_media_does_not_fall_through),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
