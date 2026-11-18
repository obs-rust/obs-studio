/* Original setter, the real private layout, and byte-exact state probes. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define obs_source_set_default_flags oracle_obs_source_set_default_flags
#include "obs-internal.h"
#include "../legacy/obs-source-default-flags.h"
#undef obs_source_set_default_flags
#include "source_default_flags_fields.inc"

typedef void (*default_flags_setter)(obs_source_t *, uint32_t);
bool oracle_source_default_flags_run(default_flags_setter setter, uint32_t initial, uint32_t current,
				     const uint32_t *values, size_t count, uint32_t *trace);
bool oracle_source_default_flags_run(default_flags_setter setter, uint32_t initial, uint32_t current,
				     const uint32_t *values, size_t count, uint32_t *trace)
{
	struct obs_source *source = calloc(1, sizeof(*source));
	struct obs_source *expected = calloc(1, sizeof(*expected));
	if (!source || !expected) {
		free(source);
		free(expected);
		return false;
	}
	source->flags = current;
	source->default_flags = initial;
	source->last_obs_ver = UINT32_C(0x12345678);
	memcpy(expected, source, sizeof(*source));
	bool valid = obs_source_default_flags_field(source) == &source->default_flags;
	for (size_t i = 0; i < count; i++) {
		expected->default_flags = values[i];
		setter(source, values[i]);
		trace[i] = source->default_flags;
		valid = valid && memcmp(source, expected, sizeof(*source)) == 0;
	}
	free(expected);
	free(source);
	return valid;
}

void oracle_source_default_flags_layout(size_t *layout);
void oracle_source_default_flags_layout(size_t *layout)
{
	layout[0] = sizeof(uint32_t);
	layout[1] = _Alignof(uint32_t);
}

struct default_flags_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void default_flags_capture_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void default_flags_capture_log(int level, const char *format, va_list args, void *param)
{
	struct default_flags_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*default_flags_get_log)(log_handler_t *, void **);
typedef void (*default_flags_set_log)(log_handler_t, void *);
bool oracle_source_default_flags_null_logs(default_flags_setter setter, default_flags_get_log get,
					   default_flags_set_log set);
bool oracle_source_default_flags_null_logs(default_flags_setter setter, default_flags_get_log get,
					   default_flags_set_log set)
{
	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	bool valid = true;
	const uint32_t values[] = {0, UINT32_MAX, UINT32_C(0x80000001)};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		struct default_flags_log rust = {0}, original = {0};
		set(default_flags_capture_log, &rust);
		setter(NULL, values[i]);
		set(default_flags_capture_log, &original);
		oracle_obs_source_set_default_flags(NULL, values[i]);
		valid = valid && rust.count == 1 && original.count == 1 && rust.level == LOG_DEBUG &&
			original.level == LOG_DEBUG &&
			strcmp(rust.message, "obs_source_set_default_flags: Null 'source' parameter") == 0 &&
			strcmp(rust.message, original.message) == 0;
	}
	set(old_handler, old_param);
	return valid;
}
