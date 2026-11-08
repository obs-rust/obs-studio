/* Original C bodies and automatically discovered private field adapter.
 * No intentional differences or excluded inputs. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_flags oracle_obs_source_get_flags
#define obs_source_enabled oracle_obs_source_enabled
#define obs_source_get_last_obs_version oracle_obs_source_get_last_obs_version
#include "obs-source-state-getters.c.h"
#undef obs_source_get_flags
#undef obs_source_enabled
#undef obs_source_get_last_obs_version
#include "source_state_getter_fields.inc"

void oracle_source_state_layout(size_t *layout);
void oracle_source_state_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_state_getter_fields);
	layout[1] = _Alignof(struct obs_source_state_getter_fields);
	layout[2] = offsetof(struct obs_source_state_getter_fields, flags);
	layout[3] = offsetof(struct obs_source_state_getter_fields, enabled);
	layout[4] = offsetof(struct obs_source_state_getter_fields, last_obs_version);
	layout[5] = sizeof(uint32_t);
	layout[6] = _Alignof(uint32_t);
	layout[7] = sizeof(bool);
	layout[8] = _Alignof(bool);
}
struct source_state_fixture {
	struct obs_source source;
	unsigned char snapshot[sizeof(struct obs_source)];
};
void *oracle_source_state_new(uint32_t flags, bool enabled, uint32_t version, uint32_t default_flags, bool hidden);
void *oracle_source_state_new(uint32_t flags, bool enabled, uint32_t version, uint32_t default_flags, bool hidden)
{
	struct source_state_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.flags = flags;
		fixture->source.enabled = enabled;
		fixture->source.last_obs_ver = version;
		fixture->source.default_flags = default_flags;
		fixture->source.temp_removed = hidden;
		memcpy(fixture->snapshot, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}
bool oracle_source_state_unchanged(const void *source);
bool oracle_source_state_unchanged(const void *source)
{
	const struct source_state_fixture *fixture = source;
	return memcmp(&fixture->source, fixture->snapshot, sizeof(fixture->source)) == 0;
}
void oracle_source_state_free(void *source);
void oracle_source_state_free(void *source)
{
	free(source);
}

struct source_state_log {
	unsigned count;
	int level;
	char message[128];
};

/* Like the other oracle printf forwarders, retain format checking at the call site. */
#if defined(__GNUC__) || defined(__clang__)
static void source_state_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif

static void source_state_capture(int level, const char *format, va_list args, void *param)
{
	struct source_state_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef uint32_t (*state_u32_getter)(const obs_source_t *source);
typedef bool (*state_bool_getter)(const obs_source_t *source);
bool oracle_source_state_null_log(state_u32_getter flags, state_bool_getter enabled, state_u32_getter version);
bool oracle_source_state_null_log(state_u32_getter flags, state_bool_getter enabled, state_u32_getter version)
{
	struct source_state_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(source_state_capture, &log);
	bool flags_ok = flags(NULL) == 0 && log.count == 1 && log.level == LOG_DEBUG &&
			strcmp(log.message, "obs_source_get_flags: Null 'source' parameter") == 0;
	bool enabled_ok = !enabled(NULL) && log.count == 2 && log.level == LOG_DEBUG &&
			  strcmp(log.message, "obs_source_enabled: Null 'source' parameter") == 0;
	bool version_ok = version(NULL) == 0 && log.count == 3 && log.level == LOG_DEBUG &&
			  strcmp(log.message, "obs_source_get_last_obs_version: Null 'source' parameter") == 0;
	base_set_log_handler(old_handler, old_param);
	return flags_ok && enabled_ok && version_ok;
}
