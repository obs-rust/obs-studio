/* Original getters and the production adapter use the real private header. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_has_roi oracle_obs_encoder_has_roi
#define obs_encoder_get_roi_increment oracle_obs_encoder_get_roi_increment
#include "obs-encoder-roi-state.c.h"
#undef obs_encoder_has_roi
#undef obs_encoder_get_roi_increment
#include "encoder_roi_state_fields.inc"

extern bool obs_encoder_has_roi(const obs_encoder_t *encoder);
extern uint32_t obs_encoder_get_roi_increment(const obs_encoder_t *encoder);

void oracle_encoder_roi_state_layout(size_t *layout);
void oracle_encoder_roi_state_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_roi_state_fields);
	layout[1] = _Alignof(struct obs_encoder_roi_state_fields);
	layout[2] = offsetof(struct obs_encoder_roi_state_fields, count);
	layout[3] = offsetof(struct obs_encoder_roi_state_fields, increment);
}

struct roi_state_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};
void *oracle_encoder_roi_state_new(size_t count, uint32_t increment);
void *oracle_encoder_roi_state_new(size_t count, uint32_t increment)
{
	struct roi_state_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.roi.num = count;
		fixture->encoder.roi_increment = increment;
		fixture->encoder.encoded_frames = UINT32_MAX;
		fixture->encoder.context.name = "independent-name";
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_roi_state_unchanged(void *encoder);
bool oracle_encoder_roi_state_unchanged(void *encoder)
{
	struct roi_state_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}
void oracle_encoder_roi_state_free(void *encoder);
void oracle_encoder_roi_state_free(void *encoder)
{
	free(encoder);
}

struct roi_state_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void roi_state_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void roi_state_capture(int level, const char *format, va_list args, void *param)
{
	struct roi_state_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_encoder_roi_state_null_log(bool rust);
bool oracle_encoder_roi_state_null_log(bool rust)
{
	struct roi_state_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(roi_state_capture, &log);
	bool ok = !(rust ? obs_encoder_has_roi(NULL) : oracle_obs_encoder_has_roi(NULL));
	ok &= log.count == 1 && log.level == LOG_DEBUG &&
	      strcmp(log.message, "obs_encoder_has_roi: Null 'encoder' parameter") == 0;
	log.count = 0;
	ok &= (rust ? obs_encoder_get_roi_increment(NULL) : oracle_obs_encoder_get_roi_increment(NULL)) == 0;
	ok &= log.count == 1 && log.level == LOG_DEBUG &&
	      strcmp(log.message, "obs_encoder_get_roi_increment: Null 'encoder' parameter") == 0;
	base_set_log_handler(old_handler, old_param);
	return ok;
}
