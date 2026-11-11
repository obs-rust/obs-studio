/* Oracle for #4627: the original getters unit with its public names renamed
 * so they link next to the Rust shims, a test double for the unity-local
 * helper (the original lives in obs-output.c.h and delegates to
 * obs_output_slots_first_video, mirrored here), and the discovered field
 * adapter over the real private output layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdbool.h>

#define obs_output_get_video_encoder oracle_obs_output_get_video_encoder
#define obs_output_get_audio_encoder oracle_obs_output_get_audio_encoder
#define obs_output_get_video_encoder2 oracle_obs_output_get_video_encoder2
#define obs_output_slots_audio_slot oracle_obs_output_slots_audio_slot
#define obs_output_slots_first_video oracle_obs_output_slots_first_video

#include "obs-output-encoder-getters.c.h"

#undef obs_output_slots_first_video
#undef obs_output_slots_audio_slot
#undef obs_output_get_audio_encoder
#undef obs_output_get_video_encoder2
#undef obs_output_get_video_encoder

/* Test double for the unity-local helper: same body as obs-output.c.h,
 * calling the C slots helper through its oracle-only name. */
static bool get_first_video_encoder_index(const struct obs_output *output, size_t *index)
{
	if (!index)
		return false;

	uint32_t occupied = 0;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (output->video_encoders[i])
			occupied |= (uint32_t)1 << i;
	}

	/* The unit's own #define renamed its call to oracle_obs_output_slots_first_video;
	 * this definition sits outside that unit, so call the oracle name directly. */
	return oracle_obs_output_slots_first_video(occupied, index);
}

#include "output_encoder_getters_fields.inc"

struct output_encoder_getters_fixture {
	struct obs_output output;
	struct obs_output before;
	struct obs_encoder vencs[MAX_OUTPUT_VIDEO_ENCODERS];
	struct obs_encoder aencs[MAX_OUTPUT_AUDIO_ENCODERS];
};

void *oracle_output_encoder_getters_new(uint32_t vmask, uint32_t amask, uint32_t flags);
void *oracle_output_encoder_getters_new(uint32_t vmask, uint32_t amask, uint32_t flags)
{
	struct output_encoder_getters_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++)
			fixture->output.video_encoders[idx] =
				(vmask & (1u << idx)) ? &fixture->vencs[idx] : NULL;
		for (size_t idx = 0; idx < MAX_OUTPUT_AUDIO_ENCODERS; idx++)
			fixture->output.audio_encoders[idx] =
				(amask & (1u << idx)) ? &fixture->aencs[idx] : NULL;
		/* Flags sit in the struct so the getters' failure to read them is
		 * observable: they are set here but must not affect any result. */
		fixture->output.info.flags = flags;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	/* The fixture handle doubles as the output pointer: obs_output leads. */
	return fixture;
}

obs_encoder_t *oracle_output_encoder_getters_expected_video(const void *handle);
obs_encoder_t *oracle_output_encoder_getters_expected_video(const void *handle)
{
	const struct output_encoder_getters_fixture *fixture = handle;
	for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++)
		if (fixture->before.video_encoders[idx])
			return fixture->before.video_encoders[idx];
	return NULL;
}

obs_encoder_t *oracle_output_encoder_getters_expected_audio(const void *handle, size_t idx);
obs_encoder_t *oracle_output_encoder_getters_expected_audio(const void *handle, size_t idx)
{
	const struct output_encoder_getters_fixture *fixture = handle;
	return idx < MAX_OUTPUT_AUDIO_ENCODERS ? fixture->before.audio_encoders[idx] : NULL;
}

bool oracle_output_encoder_getters_unchanged(const void *handle);
bool oracle_output_encoder_getters_unchanged(const void *handle)
{
	const struct output_encoder_getters_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}

void oracle_output_encoder_getters_free(void *handle);
void oracle_output_encoder_getters_free(void *handle)
{
	free(handle);
}

struct output_encoder_getters_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_encoder_getters_capture(int level, const char *format, va_list args, void *param)
		__attribute__((format(printf, 2, 0)));
#endif
static void output_encoder_getters_capture(int level, const char *format, va_list args, void *param)
{
	struct output_encoder_getters_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef obs_encoder_t *(*output_encoder_getters_video_getter)(const obs_output_t *);
typedef obs_encoder_t *(*output_encoder_getters_audio_getter)(const obs_output_t *, size_t);
typedef void (*output_encoder_getters_get_log)(log_handler_t *, void **);
typedef void (*output_encoder_getters_set_log)(log_handler_t, void *);

bool oracle_output_encoder_getters_video_logs(output_encoder_getters_video_getter getter,
					     output_encoder_getters_get_log get_log,
					     output_encoder_getters_set_log set_log);
bool oracle_output_encoder_getters_video_logs(output_encoder_getters_video_getter getter,
					     output_encoder_getters_get_log get_log,
					     output_encoder_getters_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_encoder_getters_log r = {0}, c = {0};
	set_log(output_encoder_getters_capture, &r);
	obs_encoder_t *rust_value = getter(NULL);
	set_log(output_encoder_getters_capture, &c);
	obs_encoder_t *c_value = oracle_obs_output_get_video_encoder(NULL);
	set_log(old_handler, old_param);
	return !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_output_get_video_encoder: Null 'output' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}

bool oracle_output_encoder_getters_audio_logs(output_encoder_getters_audio_getter getter,
                                             output_encoder_getters_get_log get_log,
                                             output_encoder_getters_set_log set_log, size_t idx);
bool oracle_output_encoder_getters_audio_logs(output_encoder_getters_audio_getter getter,
                                             output_encoder_getters_get_log get_log,
                                             output_encoder_getters_set_log set_log, size_t idx)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_encoder_getters_log r = {0}, c = {0};
	set_log(output_encoder_getters_capture, &r);
	obs_encoder_t *rust_value = getter(NULL, idx);
	set_log(output_encoder_getters_capture, &c);
	obs_encoder_t *c_value = oracle_obs_output_get_audio_encoder(NULL, idx);
	set_log(old_handler, old_param);
	return !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_output_get_audio_encoder: Null 'output' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
