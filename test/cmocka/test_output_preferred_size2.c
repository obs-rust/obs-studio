/* Direct actual-library characterization of the indexed preferred-size setter.
 * Preserve these assertions unchanged through the later Rust swap (Refs #5666).
 * Stack fixtures own no allocated state; no OBS startup or renderer is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct size_log {
	unsigned calls;
	int level;
	char message[256];
};

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct size_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static struct size_log set_size(obs_output_t *output, uint32_t width, uint32_t height, size_t idx)
{
	log_handler_t old_handler;
	void *old_param;
	struct size_log log = {0};
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, &log);
	obs_output_set_preferred_size2(output, width, height, idx);
	base_set_log_handler(old_handler, old_param);
	return log;
}

static void init_output(struct obs_output *output, uint32_t flags)
{
	memset(output, 0, sizeof(*output));
	output->context.name = "indexed-size";
	output->info.flags = flags;
	output->scaled_width = 37;
	output->scaled_height = 53;
}

static void null_output_diagnostic(void **state)
{
	(void)state;
	struct size_log log = set_size(NULL, 0, UINT32_MAX, SIZE_MAX);
	assert_int_equal(log.calls, 1);
	assert_int_equal(log.level, LOG_DEBUG);
	assert_string_equal(log.message, "obs_output_set_preferred_size2: Null 'output' parameter");
}

static void video_gate_precedes_index_and_active(void **state)
{
	(void)state;
	const uint32_t flags[] = {0, OBS_OUTPUT_AUDIO, OBS_OUTPUT_AUDIO | OBS_OUTPUT_ENCODED};
	const size_t indices[] = {0, 1, MAX_OUTPUT_VIDEO_ENCODERS, SIZE_MAX};
	struct obs_output output;
	unsigned char before[sizeof(output)];
	for (size_t f = 0; f < sizeof(flags) / sizeof(flags[0]); f++) {
		init_output(&output, flags[f]);
		for (unsigned active = 0; active < 2; active++) {
			output.active = active != 0;
			memcpy(before, &output, sizeof(output));
			for (size_t i = 0; i < sizeof(indices) / sizeof(indices[0]); i++) {
				struct size_log log = set_size(&output, 0, UINT32_MAX, indices[i]);
				assert_int_equal(log.calls, 1);
				assert_int_equal(log.level, LOG_WARNING);
				assert_string_equal(
					log.message,
					"Output 'indexed-size': Tried to use obs_output_set_preferred_size2 on a non-video output");
				assert_memory_equal(&output, before, sizeof(output));
			}
		}
	}
}

static void invalid_index_is_silent_before_active(void **state)
{
	(void)state;
	const size_t indices[] = {MAX_OUTPUT_VIDEO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS + 1, SIZE_MAX};
	struct obs_output output;
	unsigned char before[sizeof(output)];
	for (unsigned encoded = 0; encoded < 2; encoded++) {
		init_output(&output, OBS_OUTPUT_VIDEO | (encoded ? OBS_OUTPUT_ENCODED : 0));
		for (unsigned active = 0; active < 2; active++) {
			output.active = active != 0;
			memcpy(before, &output, sizeof(output));
			for (size_t i = 0; i < sizeof(indices) / sizeof(indices[0]); i++) {
				struct size_log log = set_size(&output, 1919, 1079, indices[i]);
				assert_int_equal(log.calls, 0);
				assert_memory_equal(&output, before, sizeof(output));
			}
		}
	}
}

static void active_video_rejects_every_valid_slot(void **state)
{
	(void)state;
	struct obs_output output;
	struct obs_encoder encoder = {0};
	unsigned char before[sizeof(output)];
	unsigned char encoder_before[sizeof(encoder)];
	encoder.info.type = OBS_ENCODER_VIDEO;
	encoder.scaled_width = 11;
	encoder.scaled_height = 13;
	memcpy(encoder_before, &encoder, sizeof(encoder));
	for (unsigned encoded = 0; encoded < 2; encoded++) {
		init_output(&output, OBS_OUTPUT_VIDEO | (encoded ? OBS_OUTPUT_ENCODED : 0));
		output.active = true;
		for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++)
			output.video_encoders[idx] = &encoder;
		memcpy(before, &output, sizeof(output));
		for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++) {
			struct size_log log = set_size(&output, 1919, 1079, idx);
			assert_int_equal(log.calls, 1);
			assert_int_equal(log.level, LOG_WARNING);
			assert_string_equal(
				log.message,
				"output 'indexed-size': Cannot set the preferred resolution while the output is active");
			assert_memory_equal(&output, before, sizeof(output));
			assert_memory_equal(&encoder, encoder_before, sizeof(encoder));
		}
	}
}

static void raw_sizes_only_write_slot_zero(void **state)
{
	(void)state;
	struct obs_output output, expected;
	struct obs_encoder encoder = {0};
	unsigned char encoder_before[sizeof(encoder)];
	const uint32_t sizes[][2] = {{1, 3}, {1919, 1079}, {UINT32_MAX, UINT32_MAX - 1}, {0, 701}, {905, 0}, {0, 0}};
	encoder.info.type = OBS_ENCODER_VIDEO;
	encoder.scaled_width = 11;
	encoder.scaled_height = 13;
	memcpy(encoder_before, &encoder, sizeof(encoder));
	init_output(&output, OBS_OUTPUT_VIDEO);
	/* Even populated encoder slots are ignored by a raw output. */
	for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++)
		output.video_encoders[idx] = &encoder;
	for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++) {
		for (size_t s = 0; s < sizeof(sizes) / sizeof(sizes[0]); s++) {
			memcpy(&expected, &output, sizeof(output));
			if (idx == 0) {
				expected.scaled_width = sizes[s][0];
				expected.scaled_height = sizes[s][1];
			}
			struct size_log log = set_size(&output, sizes[s][0], sizes[s][1], idx);
			assert_int_equal(log.calls, 0);
			assert_memory_equal(&output, &expected, sizeof(output));
			assert_memory_equal(&encoder, encoder_before, sizeof(encoder));
		}
	}
}

static void encoded_sizes_target_exact_slot(void **state)
{
	(void)state;
	struct obs_output output, expected;
	struct obs_encoder encoders[MAX_OUTPUT_VIDEO_ENCODERS] = {0};
	struct obs_encoder expected_encoders[MAX_OUTPUT_VIDEO_ENCODERS];
	const uint32_t sizes[][2] = {{1919, 1079}, {0, UINT32_MAX}, {UINT32_MAX, 0}, {0, 0}};
	init_output(&output, OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED);
	for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++) {
		encoders[idx].info.type = OBS_ENCODER_VIDEO;
		encoders[idx].context.name = "slot-encoder";
		encoders[idx].scaled_width = (uint32_t)idx + 11;
		encoders[idx].scaled_height = (uint32_t)idx + 13;
		output.video_encoders[idx] = &encoders[idx];
	}
	for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++) {
		for (size_t s = 0; s < sizeof(sizes) / sizeof(sizes[0]); s++) {
			memcpy(&expected, &output, sizeof(output));
			memcpy(expected_encoders, encoders, sizeof(encoders));
			if (idx == 0) {
				expected.scaled_width = sizes[s][0];
				expected.scaled_height = sizes[s][1];
			}
			expected_encoders[idx].scaled_width = sizes[s][0];
			expected_encoders[idx].scaled_height = sizes[s][1];
			struct size_log log = set_size(&output, sizes[s][0], sizes[s][1], idx);
			assert_int_equal(log.calls, 0);
			assert_memory_equal(&output, &expected, sizeof(output));
			assert_memory_equal(encoders, expected_encoders, sizeof(encoders));
		}
		/* Missing selected encoder is a silent no-op beyond the slot-zero write. */
		output.video_encoders[idx] = NULL;
		memcpy(&expected, &output, sizeof(output));
		memcpy(expected_encoders, encoders, sizeof(encoders));
		if (idx == 0) {
			expected.scaled_width = 320;
			expected.scaled_height = 240;
		}
		struct size_log log = set_size(&output, 320, 240, idx);
		assert_int_equal(log.calls, 0);
		assert_memory_equal(&output, &expected, sizeof(output));
		assert_memory_equal(encoders, expected_encoders, sizeof(encoders));
		output.video_encoders[idx] = &encoders[idx];
	}
}

static void encoder_refusal_keeps_prior_slot_zero_write(void **state)
{
	(void)state;
	const char *messages[] = {
		"obs_encoder_set_scaled_size: encoder 'slot-encoder' is not a video encoder",
		"encoder 'slot-encoder': Cannot set the scaled resolution while the encoder is active",
		"encoder 'slot-encoder': Cannot set the scaled resolution after the encoder has been initialized",
	};
	struct obs_output output, expected;
	struct obs_encoder encoder;
	unsigned char encoder_before[sizeof(encoder)];
	for (unsigned gate = 0; gate < 3; gate++) {
		memset(&encoder, 0, sizeof(encoder));
		encoder.context.name = "slot-encoder";
		encoder.info.type = gate == 0 ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		encoder.active = gate == 1;
		encoder.initialized = gate == 2;
		encoder.scaled_width = 11;
		encoder.scaled_height = 13;
		memcpy(encoder_before, &encoder, sizeof(encoder));
		init_output(&output, OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED);
		for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++)
			output.video_encoders[idx] = &encoder;
		for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++) {
			memcpy(&expected, &output, sizeof(output));
			if (idx == 0) {
				expected.scaled_width = 1919;
				expected.scaled_height = 1079;
			}
			struct size_log log = set_size(&output, 1919, 1079, idx);
			assert_int_equal(log.calls, 1);
			assert_int_equal(log.level, LOG_WARNING);
			assert_string_equal(log.message, messages[gate]);
			/* Characterized, not endorsed: no rollback after encoder refusal. */
			assert_memory_equal(&output, &expected, sizeof(output));
			assert_memory_equal(&encoder, encoder_before, sizeof(encoder));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_output_diagnostic),
		cmocka_unit_test(video_gate_precedes_index_and_active),
		cmocka_unit_test(invalid_index_is_silent_before_active),
		cmocka_unit_test(active_video_rejects_every_valid_slot),
		cmocka_unit_test(raw_sizes_only_write_slot_zero),
		cmocka_unit_test(encoded_sizes_target_exact_slot),
		cmocka_unit_test(encoder_refusal_keeps_prior_slot_zero_write),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
