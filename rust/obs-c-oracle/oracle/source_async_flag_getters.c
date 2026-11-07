/* Original getters and adapter over the real private source layout.
 * No intended behavior differences or exclusions. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_async_unbuffered oracle_obs_source_async_unbuffered
#define obs_source_async_decoupled oracle_obs_source_async_decoupled
#include "obs-source-async-flag-getters.c.h"
#undef obs_source_async_unbuffered
#undef obs_source_async_decoupled
#include "source_async_flag_fields.inc"

extern bool obs_source_async_unbuffered(const obs_source_t *source);
extern bool obs_source_async_decoupled(const obs_source_t *source);

void oracle_source_async_flag_layout(size_t *layout);
void oracle_source_async_flag_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_async_flag_fields);
	layout[1] = _Alignof(struct obs_source_async_flag_fields);
	layout[2] = offsetof(struct obs_source_async_flag_fields, unbuffered);
	layout[3] = offsetof(struct obs_source_async_flag_fields, decoupled);
	layout[4] = sizeof(bool);
	layout[5] = _Alignof(bool);
	layout[6] = sizeof(bool);
	layout[7] = _Alignof(bool);
}

struct async_flag_fixture {
	struct obs_source source;
	unsigned char snapshot[sizeof(struct obs_source)];
};

void *oracle_source_async_flag_new(bool unbuffered, bool decoupled);
void *oracle_source_async_flag_new(bool unbuffered, bool decoupled)
{
	struct async_flag_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.async_unbuffered = unbuffered;
		fixture->source.async_decoupled = decoupled;
		fixture->source.async_width = 17;
		fixture->source.async_height = 29;
		memcpy(fixture->snapshot, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

bool oracle_source_async_flag_unchanged(const void *source);
bool oracle_source_async_flag_unchanged(const void *source)
{
	const struct async_flag_fixture *fixture = source;
	return memcmp(&fixture->source, fixture->snapshot, sizeof(fixture->source)) == 0;
}

void oracle_source_async_flag_free(void *source);
void oracle_source_async_flag_free(void *source)
{
	free(source);
}

struct async_flag_log {
	unsigned count;
	int level;
	char message[128];
};

/* Like the other oracle printf forwarders, retain format checking at the call site. */
#if defined(__GNUC__) || defined(__clang__)
static void async_flag_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif

static void async_flag_capture(int level, const char *format, va_list args, void *param)
{
	struct async_flag_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_source_async_flag_null_log(bool rust);
bool oracle_source_async_flag_null_log(bool rust)
{
	struct async_flag_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(async_flag_capture, &log);
	bool unbuffered = rust ? obs_source_async_unbuffered(NULL) : oracle_obs_source_async_unbuffered(NULL);
	bool unbuffered_ok = !unbuffered && log.count == 1 && log.level == LOG_DEBUG &&
			     strcmp(log.message, "obs_source_async_unbuffered: Null 'source' parameter") == 0;
	bool decoupled = rust ? obs_source_async_decoupled(NULL) : oracle_obs_source_async_decoupled(NULL);
	base_set_log_handler(old_handler, old_param);
	return unbuffered_ok && !decoupled && log.count == 2 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_source_async_decoupled: Null 'source' parameter") == 0;
}
