/* Extracted original C and discovered real-header field adapter. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_get_frame_rate_divisor oracle_obs_encoder_get_frame_rate_divisor
#define obs_encoder_get_name oracle_encoder_divisor_name
#include "obs-encoder-frame-rate-divisor.c.h"
#undef obs_encoder_get_name
#undef obs_encoder_get_frame_rate_divisor
#include "encoder_frame_rate_divisor_fields.inc"

const char *oracle_encoder_divisor_name(const obs_encoder_t *encoder)
{
	return encoder->context.name;
}

void oracle_encoder_divisor_layout(size_t *layout);
void oracle_encoder_divisor_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_frame_rate_divisor_fields);
	layout[1] = _Alignof(struct obs_encoder_frame_rate_divisor_fields);
	layout[2] = offsetof(struct obs_encoder_frame_rate_divisor_fields, type);
	layout[3] = offsetof(struct obs_encoder_frame_rate_divisor_fields, divisor);
	layout[4] = offsetof(struct obs_encoder_frame_rate_divisor_fields, name);
	layout[5] = sizeof(enum obs_encoder_type);
	layout[6] = _Alignof(enum obs_encoder_type);
	layout[7] = OBS_ENCODER_AUDIO;
	layout[8] = OBS_ENCODER_VIDEO;
}

struct divisor_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
	char name[64];
};
void *oracle_encoder_divisor_new(int type, uint32_t divisor, const char *name);
void *oracle_encoder_divisor_new(int type, uint32_t divisor, const char *name)
{
	struct divisor_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.info.type = (enum obs_encoder_type)type;
		fixture->encoder.orig_info.type = type == OBS_ENCODER_VIDEO ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		snprintf(fixture->name, sizeof(fixture->name), "%s", name);
		fixture->encoder.context.name = fixture->name;
		fixture->encoder.frame_rate_divisor = divisor;
		fixture->encoder.scaled_width = ~divisor;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_encoder_divisor_unchanged(void *encoder);
bool oracle_encoder_divisor_unchanged(void *encoder)
{
	struct divisor_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}
void oracle_encoder_divisor_free(void *encoder);
void oracle_encoder_divisor_free(void *encoder)
{
	free(encoder);
}

struct divisor_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void divisor_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void divisor_capture(int level, const char *format, va_list args, void *param)
{
	struct divisor_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef uint32_t (*divisor_getter)(const obs_encoder_t *);
/* Receive the Rust callback: no production-symbol dependency in this C object (COFF). */
bool oracle_encoder_divisor_probe(const obs_encoder_t *encoder, divisor_getter getter);
bool oracle_encoder_divisor_probe(const obs_encoder_t *encoder, divisor_getter getter)
{
	struct divisor_log rust_log = {0}, c_log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(divisor_capture, &rust_log);
	uint32_t rust_value = getter(encoder);
	base_set_log_handler(divisor_capture, &c_log);
	uint32_t c_value = oracle_obs_encoder_get_frame_rate_divisor(encoder);
	base_set_log_handler(old_handler, old_param);
	return rust_value == c_value && rust_log.count == c_log.count && rust_log.level == c_log.level &&
	       strcmp(rust_log.message, c_log.message) == 0;
}
