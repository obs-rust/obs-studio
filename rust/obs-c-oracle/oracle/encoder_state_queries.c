/* Original extracted getters and discovered field adapter over real C layouts. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_active oracle_obs_encoder_active
#define obs_encoder_paused oracle_obs_encoder_paused
#include "obs-encoder-state-queries.c.h"
#undef obs_encoder_paused
#undef obs_encoder_active
#include "encoder_state_queries_fields.inc"

struct encoder_state_queries_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
};

void *oracle_encoder_state_queries_new(bool active, bool paused, bool initialized);
void *oracle_encoder_state_queries_new(bool active, bool paused, bool initialized)
{
	struct encoder_state_queries_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		os_atomic_set_bool(&fixture->encoder.active, active);
		os_atomic_set_bool(&fixture->encoder.paused, paused);
		fixture->encoder.initialized = initialized;
		memcpy(&fixture->before, &fixture->encoder, sizeof(fixture->encoder));
	}
	return fixture;
}

bool oracle_encoder_state_queries_unchanged(const void *handle);
bool oracle_encoder_state_queries_unchanged(const void *handle)
{
	const struct encoder_state_queries_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->encoder, sizeof(fixture->encoder)) == 0;
}

void oracle_encoder_state_queries_free(void *handle);
void oracle_encoder_state_queries_free(void *handle)
{
	free(handle);
}

bool oracle_encoder_state_queries_addresses(const obs_encoder_t *encoder);
bool oracle_encoder_state_queries_addresses(const obs_encoder_t *encoder)
{
	struct obs_encoder_state_queries_fields fields = obs_encoder_state_queries_fields(encoder);
	return fields.active == &encoder->active && fields.paused == &encoder->paused;
}

void oracle_encoder_state_queries_layout(size_t *layout);
void oracle_encoder_state_queries_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_state_queries_fields);
	layout[1] = _Alignof(struct obs_encoder_state_queries_fields);
	layout[2] = offsetof(struct obs_encoder_state_queries_fields, active);
	layout[3] = offsetof(struct obs_encoder_state_queries_fields, paused);
	layout[4] = sizeof(((struct obs_encoder *)0)->active);
	layout[5] = _Alignof(bool);
}

struct encoder_state_queries_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void encoder_state_queries_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void encoder_state_queries_capture(int level, const char *format, va_list args, void *param)
{
	struct encoder_state_queries_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef bool (*encoder_state_queries_getter)(const obs_encoder_t *);
typedef void (*encoder_state_queries_get_log)(log_handler_t *, void **);
typedef void (*encoder_state_queries_set_log)(log_handler_t, void *);
bool oracle_encoder_state_queries_logs(encoder_state_queries_getter active, encoder_state_queries_getter paused,
				       encoder_state_queries_get_log get_log, encoder_state_queries_set_log set_log);
bool oracle_encoder_state_queries_logs(encoder_state_queries_getter active, encoder_state_queries_getter paused,
				       encoder_state_queries_get_log get_log, encoder_state_queries_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	encoder_state_queries_getter rust[] = {active, paused};
	encoder_state_queries_getter original[] = {oracle_obs_encoder_active, oracle_obs_encoder_paused};
	const char *expected[] = {"obs_encoder_active: Null 'encoder' parameter",
				  "obs_encoder_paused: Null 'encoder' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		struct encoder_state_queries_log r = {0}, c = {0};
		set_log(encoder_state_queries_capture, &r);
		bool rust_value = rust[i](NULL);
		set_log(encoder_state_queries_capture, &c);
		bool c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
