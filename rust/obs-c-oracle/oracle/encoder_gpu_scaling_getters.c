/* Original C getters with a discovered real-header field adapter. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_gpu_scaling_enabled oracle_obs_encoder_gpu_scaling_enabled
#define obs_encoder_get_scale_type oracle_obs_encoder_get_scale_type
#define obs_encoder_get_name oracle_encoder_gpu_scaling_name
#include "obs-encoder-gpu-scaling-getters.c.h"
#undef obs_encoder_get_name
#undef obs_encoder_get_scale_type
#undef obs_encoder_gpu_scaling_enabled
#include "encoder_gpu_scaling_getters_fields.inc"

const char *oracle_encoder_gpu_scaling_name(const obs_encoder_t *encoder)
{
	return encoder->context.name;
}

void oracle_encoder_gpu_scaling_layout(size_t *layout);
void oracle_encoder_gpu_scaling_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_gpu_scaling_getters_fields);
	layout[1] = _Alignof(struct obs_encoder_gpu_scaling_getters_fields);
	layout[2] = offsetof(struct obs_encoder_gpu_scaling_getters_fields, type);
	layout[3] = offsetof(struct obs_encoder_gpu_scaling_getters_fields, scale);
	layout[4] = offsetof(struct obs_encoder_gpu_scaling_getters_fields, name);
	layout[5] = sizeof(enum obs_encoder_type);
	layout[6] = _Alignof(enum obs_encoder_type);
	layout[7] = sizeof(enum obs_scale_type);
	layout[8] = _Alignof(enum obs_scale_type);
	layout[9] = OBS_ENCODER_AUDIO;
	layout[10] = OBS_ENCODER_VIDEO;
	layout[11] = OBS_SCALE_DISABLE;
	layout[12] = OBS_SCALE_AREA;
}

struct gpu_scaling_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
	char name[64];
};
void *oracle_encoder_gpu_scaling_new(int type, int scale, uint32_t width, uint32_t height);
void *oracle_encoder_gpu_scaling_new(int type, int scale, uint32_t width, uint32_t height)
{
	struct gpu_scaling_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = (enum obs_encoder_type)type;
		fixture->encoder.orig_info.type = type == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		snprintf(fixture->name, sizeof(fixture->name), "%s", "gpu-scaling-%s");
		fixture->encoder.context.name = fixture->name;
		fixture->encoder.gpu_scale_type = (enum obs_scale_type)scale;
		fixture->encoder.scaled_width = width;
		fixture->encoder.scaled_height = height;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_gpu_scaling_unchanged(void *encoder);
bool oracle_encoder_gpu_scaling_unchanged(void *encoder)
{
	struct gpu_scaling_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0 &&
	       strcmp(fixture->name, "gpu-scaling-%s") == 0;
}
void oracle_encoder_gpu_scaling_free(void *encoder);
void oracle_encoder_gpu_scaling_free(void *encoder)
{
	free(encoder);
}

struct gpu_scaling_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void gpu_scaling_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void gpu_scaling_capture(int level, const char *format, va_list args, void *param)
{
	struct gpu_scaling_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

/* Receive callbacks to avoid production libobs symbol dependencies in this object. */
typedef bool (*gpu_scaling_enabled_getter)(obs_encoder_t *);
typedef enum obs_scale_type (*gpu_scaling_type_getter)(obs_encoder_t *);
bool oracle_encoder_gpu_scaling_probe(obs_encoder_t *encoder, gpu_scaling_enabled_getter enabled,
				      gpu_scaling_type_getter scale_type);
bool oracle_encoder_gpu_scaling_probe(obs_encoder_t *encoder, gpu_scaling_enabled_getter enabled,
				      gpu_scaling_type_getter scale_type)
{
	struct gpu_scaling_log rust_log = {0}, c_log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(gpu_scaling_capture, &rust_log);
	bool rust_enabled = enabled(encoder);
	base_set_log_handler(gpu_scaling_capture, &c_log);
	bool c_enabled = oracle_obs_encoder_gpu_scaling_enabled(encoder);
	bool enabled_equal = rust_enabled == c_enabled && rust_log.count == c_log.count &&
			     rust_log.level == c_log.level && strcmp(rust_log.message, c_log.message) == 0;
	memset(&rust_log, 0, sizeof(rust_log));
	memset(&c_log, 0, sizeof(c_log));
	base_set_log_handler(gpu_scaling_capture, &rust_log);
	enum obs_scale_type rust_type = scale_type(encoder);
	base_set_log_handler(gpu_scaling_capture, &c_log);
	enum obs_scale_type c_type = oracle_obs_encoder_get_scale_type(encoder);
	base_set_log_handler(old_handler, old_param);
	return enabled_equal && rust_type == c_type && rust_log.count == c_log.count && rust_log.level == c_log.level &&
	       strcmp(rust_log.message, c_log.message) == 0;
}
