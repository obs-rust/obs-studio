/* Original setter and real private-layout adapter; callback records delegation. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_set_name oracle_obs_encoder_set_name
#include "obs-encoder-set-name.c.h"
#undef obs_encoder_set_name
#include "encoder_set_name_fields.inc"

struct set_name_fixture {
	struct obs_encoder encoder;
	unsigned calls;
	const char *argument;
};

/* A callback spy, not a replacement for Tier 2's real context implementation. */
void obs_context_data_setname(struct obs_context_data *context, const char *name)
{
	struct set_name_fixture *fixture = (void *)context;
	fixture->calls++;
	fixture->argument = name;
	/* Borrow the pointer without modifying caller-owned string bytes. */
	memcpy(&context->name, &name, sizeof(name));
}

void oracle_encoder_set_name_layout(size_t *layout);
void oracle_encoder_set_name_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_set_name_fields);
	layout[1] = _Alignof(struct obs_encoder_set_name_fields);
	layout[2] = offsetof(struct obs_encoder_set_name_fields, context);
	layout[3] = offsetof(struct obs_encoder_set_name_fields, name);
}

void *oracle_encoder_set_name_new(const char *name);
void *oracle_encoder_set_name_new(const char *name)
{
	struct set_name_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture)
		memcpy(&fixture->encoder.context.name, &name, sizeof(name));
	return fixture;
}

bool oracle_encoder_set_name_check(void *encoder, unsigned calls, const char *argument, const char *name);
bool oracle_encoder_set_name_check(void *encoder, unsigned calls, const char *argument, const char *name)
{
	struct set_name_fixture *fixture = encoder;
	return fixture->calls == calls && fixture->argument == argument && fixture->encoder.context.name == name;
}

void oracle_encoder_set_name_free(void *encoder);
void oracle_encoder_set_name_free(void *encoder)
{
	free(encoder);
}

typedef void (*set_name_fn)(obs_encoder_t *, const char *);
struct set_name_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void set_name_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void set_name_capture(int level, const char *format, va_list args, void *param)
{
	struct set_name_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_encoder_set_name_null_log(set_name_fn setter);
bool oracle_encoder_set_name_null_log(set_name_fn setter)
{
	struct set_name_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(set_name_capture, &log);
	setter(NULL, NULL);
	bool ok = log.count == 1 && log.level == LOG_DEBUG &&
		  strcmp(log.message, "obs_encoder_set_name: Null 'encoder' parameter") == 0;
	base_set_log_handler(old_handler, old_param);
	return ok;
}
