/* Original C getters and the production field adapter over the real private
 * output layout. obs_output_get_video_encoder2() and the slot rules are the
 * original C of their own oracles (output_video_encoder_getter.c,
 * output_slots.c); get_first_video_encoder_index() is obs-output.c's body,
 * reached through its non-static wrapper as in libobs. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_video_encoder oracle_obs_output_get_video_encoder
#define obs_output_get_audio_encoder oracle_obs_output_get_audio_encoder
#define obs_output_get_video_encoder2 oracle_obs_output_get_video_encoder2
#define obs_output_slots_audio_slot oracle_obs_output_slots_audio_slot
#define obs_output_slots_first_video oracle_obs_output_slots_first_video
#define obs_output_interleave_first_video_encoder_index oracle_output_encoder_getters_first_video
#include "obs-output-encoder-getters.c.h"

/* obs-output.c's static get_first_video_encoder_index(), unchanged. */
bool obs_output_interleave_first_video_encoder_index(const obs_output_t *output, size_t *index)
{
	if (!index)
		return false;

	uint32_t occupied = 0;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (output->video_encoders[i])
			occupied |= (uint32_t)1 << i;
	}

	return obs_output_slots_first_video(occupied, index);
}
#undef obs_output_interleave_first_video_encoder_index
#undef obs_output_slots_first_video
#undef obs_output_slots_audio_slot
#undef obs_output_get_video_encoder2
#undef obs_output_get_audio_encoder
#undef obs_output_get_video_encoder
#include "output_encoder_getters_fields.inc"

struct output_encoder_getters_fixture {
	struct obs_output output;
	struct obs_output before;
	struct obs_encoder video[MAX_OUTPUT_VIDEO_ENCODERS];
	struct obs_encoder audio[MAX_OUTPUT_AUDIO_ENCODERS];
};

void *oracle_output_encoder_getters_new(uint32_t video, uint32_t audio, uint32_t flags);
void *oracle_output_encoder_getters_new(uint32_t video, uint32_t audio, uint32_t flags)
{
	struct output_encoder_getters_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++)
			fixture->output.video_encoders[idx] = (video & (1u << idx)) ? &fixture->video[idx] : NULL;
		for (size_t idx = 0; idx < MAX_OUTPUT_AUDIO_ENCODERS; idx++)
			fixture->output.audio_encoders[idx] = (audio & (1u << idx)) ? &fixture->audio[idx] : NULL;
		fixture->output.info.flags = flags;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}

/* What the test expects, from the fixture alone: the lowest occupied video
 * slot, and the audio slot at idx. */
void *oracle_output_encoder_getters_expected_video(const void *handle);
void *oracle_output_encoder_getters_expected_video(const void *handle)
{
	const struct output_encoder_getters_fixture *fixture = handle;
	for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++) {
		if (fixture->before.video_encoders[idx])
			return fixture->before.video_encoders[idx];
	}
	return NULL;
}

void *oracle_output_encoder_getters_expected_audio(const void *handle, size_t idx);
void *oracle_output_encoder_getters_expected_audio(const void *handle, size_t idx)
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

static bool output_encoder_getters_same_log(const struct output_encoder_getters_log *r,
					    const struct output_encoder_getters_log *c, const char *expected)
{
	return r->count == 1 && c->count == 1 && r->level == LOG_DEBUG && c->level == LOG_DEBUG &&
	       strcmp(r->message, expected) == 0 && strcmp(r->message, c->message) == 0;
}

typedef void *(*output_encoder_getters_video)(const obs_output_t *);
typedef void *(*output_encoder_getters_audio)(const obs_output_t *, size_t);
typedef void (*output_encoder_getters_get_log)(log_handler_t *, void **);
typedef void (*output_encoder_getters_set_log)(log_handler_t, void *);
bool oracle_output_encoder_getters_logs(output_encoder_getters_video video, output_encoder_getters_audio audio,
					output_encoder_getters_get_log get_log, output_encoder_getters_set_log set_log,
					size_t idx);
bool oracle_output_encoder_getters_logs(output_encoder_getters_video video, output_encoder_getters_audio audio,
					output_encoder_getters_get_log get_log, output_encoder_getters_set_log set_log,
					size_t idx)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_encoder_getters_log rv = {0}, cv = {0}, ra = {0}, ca = {0};
	set_log(output_encoder_getters_capture, &rv);
	void *rust_video = video(NULL);
	set_log(output_encoder_getters_capture, &cv);
	void *c_video = oracle_obs_output_get_video_encoder(NULL);
	set_log(output_encoder_getters_capture, &ra);
	void *rust_audio = audio(NULL, idx);
	set_log(output_encoder_getters_capture, &ca);
	void *c_audio = oracle_obs_output_get_audio_encoder(NULL, idx);
	set_log(old_handler, old_param);
	return !rust_video && !c_video && !rust_audio && !c_audio &&
	       output_encoder_getters_same_log(&rv, &cv, "obs_output_get_video_encoder: Null 'output' parameter") &&
	       output_encoder_getters_same_log(&ra, &ca, "obs_output_get_audio_encoder: Null 'output' parameter");
}
