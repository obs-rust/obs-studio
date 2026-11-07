/* Original C getter with oracle-only helper dependency and a callback probe. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_scaling_enabled oracle_obs_encoder_scaling_enabled
#define obs_encoder_size_scaling oracle_obs_encoder_size_scaling
#include "obs-encoder-scaling-enabled.c.h"
#undef obs_encoder_size_scaling
#undef obs_encoder_scaling_enabled
#include "encoder_scaling_enabled_fields.inc"

void oracle_encoder_scaling_layout(size_t *layout);
void oracle_encoder_scaling_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_scaling_enabled_fields);
	layout[1] = _Alignof(struct obs_encoder_scaling_enabled_fields);
	layout[2] = offsetof(struct obs_encoder_scaling_enabled_fields, width);
	layout[3] = offsetof(struct obs_encoder_scaling_enabled_fields, height);
}

struct scaling_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};
void *oracle_encoder_scaling_new(uint32_t width, uint32_t height, bool video);
void *oracle_encoder_scaling_new(uint32_t width, uint32_t height, bool video)
{
	struct scaling_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = video ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
		fixture->encoder.orig_info.type = video ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		fixture->encoder.scaled_width = width;
		fixture->encoder.scaled_height = height;
		fixture->encoder.gpu_scale_type = OBS_SCALE_DISABLE;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_scaling_unchanged(void *encoder);
bool oracle_encoder_scaling_unchanged(void *encoder)
{
	struct scaling_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}
void oracle_encoder_scaling_free(void *encoder);
void oracle_encoder_scaling_free(void *encoder)
{
	free(encoder);
}

struct scaling_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void scaling_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void scaling_capture(int level, const char *format, va_list args, void *param)
{
	struct scaling_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
typedef bool (*scaling_getter)(const obs_encoder_t *);
bool oracle_encoder_scaling_null_log(scaling_getter getter);
bool oracle_encoder_scaling_null_log(scaling_getter getter)
{
	struct scaling_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(scaling_capture, &log);
	bool value = getter(NULL);
	base_set_log_handler(old_handler, old_param);
	return !value && log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_encoder_scaling_enabled: Null 'encoder' parameter") == 0;
}
