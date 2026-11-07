/* Original getter and production field adapter, both over the real private header. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_get_last_error oracle_obs_encoder_get_last_error
#include "obs-encoder-last-error.c.h"
#undef obs_encoder_get_last_error
#include "encoder_last_error_fields.inc"

typedef const char *(*last_error_getter)(obs_encoder_t *encoder);

void oracle_encoder_last_error_layout(size_t *layout);
void oracle_encoder_last_error_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_last_error_fields);
	layout[1] = _Alignof(struct obs_encoder_last_error_fields);
	layout[2] = offsetof(struct obs_encoder_last_error_fields, message);
}

struct last_error_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};

void *oracle_encoder_last_error_new(char *message);
void *oracle_encoder_last_error_new(char *message)
{
	struct last_error_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->encoder.last_error_message = message;
		fixture->encoder.context.name = "distinct-context-name";
		fixture->encoder.encoded_frames = UINT32_MAX;
		fixture->encoder.roi_increment = UINT32_C(0x80000001);
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->before));
	}
	return fixture;
}

bool oracle_encoder_last_error_unchanged(void *encoder);
bool oracle_encoder_last_error_unchanged(void *encoder)
{
	struct last_error_fixture *fixture = encoder;
	return memcmp(&fixture->encoder, &fixture->before, sizeof(fixture->encoder)) == 0;
}

void oracle_encoder_last_error_free(void *encoder);
void oracle_encoder_last_error_free(void *encoder)
{
	free(encoder);
}

struct last_error_log {
	unsigned count;
	int level;
	char message[128];
};

#if defined(__GNUC__) || defined(__clang__)
static void last_error_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void last_error_capture(int level, const char *format, va_list args, void *param)
{
	struct last_error_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_encoder_last_error_null_log(last_error_getter getter);
bool oracle_encoder_last_error_null_log(last_error_getter getter)
{
	struct last_error_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(last_error_capture, &log);
	bool ok = getter(NULL) == NULL;
	ok &= log.count == 1 && log.level == LOG_DEBUG &&
	      strcmp(log.message, "obs_encoder_get_last_error: Null 'encoder' parameter") == 0;
	base_set_log_handler(old_handler, old_param);
	return ok;
}
