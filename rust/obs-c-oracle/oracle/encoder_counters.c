/* Original extracted C, over the real encoder layout. No behavior exclusions. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_get_encoded_frames oracle_obs_encoder_get_encoded_frames
#define obs_encoder_get_pause_offset oracle_obs_encoder_get_pause_offset
#include "obs-encoder-counter-getters.c.h"
#undef obs_encoder_get_pause_offset
#undef obs_encoder_get_encoded_frames
#include "encoder_counter_fields.inc"

extern uint32_t obs_encoder_get_encoded_frames(const obs_encoder_t *encoder);
extern uint64_t obs_encoder_get_pause_offset(const obs_encoder_t *encoder);

void oracle_encoder_counter_layout(size_t *layout);
void oracle_encoder_counter_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_counter_fields);
	layout[1] = _Alignof(struct obs_encoder_counter_fields);
	layout[2] = offsetof(struct obs_encoder_counter_fields, encoded_frames);
	layout[3] = offsetof(struct obs_encoder_counter_fields, pause_offset);
}

void *oracle_encoder_counter_new(uint32_t frames, uint64_t offset);
void *oracle_encoder_counter_new(uint32_t frames, uint64_t offset)
{
	struct obs_encoder *encoder = calloc(1, sizeof(*encoder));
	if (encoder) {
		encoder->encoded_frames = frames;
		encoder->pause.ts_offset = offset;
		encoder->pause.ts_start = 17;
		encoder->pause.ts_end = 29;
	}
	return encoder;
}

bool oracle_encoder_counter_unchanged(const obs_encoder_t *encoder, uint32_t frames, uint64_t offset);
bool oracle_encoder_counter_unchanged(const obs_encoder_t *encoder, uint32_t frames, uint64_t offset)
{
	return encoder->encoded_frames == frames && encoder->pause.ts_offset == offset &&
	       encoder->pause.ts_start == 17 && encoder->pause.ts_end == 29;
}

void oracle_encoder_counter_free(void *encoder);
void oracle_encoder_counter_free(void *encoder)
{
	free(encoder);
}

struct counter_log {
	unsigned count;
	int level;
	char message[128];
};

/* va_list printf callback, matching the format-checked log-capture oracles. */
#if defined(__GNUC__) || defined(__clang__)
static void counter_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void counter_capture(int level, const char *format, va_list args, void *param)
{
	struct counter_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_encoder_counter_null_log(bool rust);
bool oracle_encoder_counter_null_log(bool rust)
{
	struct counter_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(counter_capture, &log);
	uint32_t frames = rust ? obs_encoder_get_encoded_frames(NULL) : oracle_obs_encoder_get_encoded_frames(NULL);
	unsigned after_frames = log.count;
	uint64_t offset = rust ? obs_encoder_get_pause_offset(NULL) : oracle_obs_encoder_get_pause_offset(NULL);
	base_set_log_handler(old_handler, old_param);
	return frames == 0 && offset == 0 && after_frames == 1 && log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_output_get_encoded_frames: Null 'encoder' parameter") == 0;
}
