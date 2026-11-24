/* Original obs_source_removed() over the real private layout, and the
 * production flag adapter the Rust shim reads through
 * (obs-source-removed-fields.c.h, defined once, here). */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_removed oracle_obs_source_removed
#include "libobs/build/libobs.pch.h"
#include "source-removed.inc"
#undef obs_source_removed
#include "source_removed_fields.inc"

struct source_removed_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_removed_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_removed_capture(int level, const char *format, va_list args, void *param)
{
	struct source_removed_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef bool (*source_removed_query)(const obs_source_t *);
typedef void (*source_removed_get_log)(log_handler_t *, void **);
typedef void (*source_removed_set_log)(log_handler_t, void *);

/* One source, or NULL when present is false: every byte is noise, then
 * removed takes the given value. The given query and the original each
 * answer; both must answer alike, log alike (obs_source_removed's line once
 * for NULL, never otherwise) and leave every byte unchanged. The shared
 * answer goes to *answer. */
bool oracle_source_removed_check(source_removed_query query, source_removed_get_log get_log,
				 source_removed_set_log set_log, bool present, uint8_t noise, bool removed,
				 bool *answer);
bool oracle_source_removed_check(source_removed_query query, source_removed_get_log get_log,
				 source_removed_set_log set_log, bool present, uint8_t noise, bool removed,
				 bool *answer)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	source_removed_query queries[2] = {query, oracle_obs_source_removed};
	bool results[2];
	struct source_removed_log logs[2];
	bool equal = true;
	for (size_t side = 0; side < 2; side++) {
		struct obs_source *source = malloc(sizeof(*source));
		struct obs_source *before = malloc(sizeof(*before));
		if (!source || !before) {
			free(source);
			free(before);
			return false;
		}
		memset(source, noise, sizeof(*source));
		source->removed = removed;
		memcpy(before, source, sizeof(*source));
		memset(&logs[side], 0, sizeof(logs[side]));
		logs[side].level = -1;
		set_log(source_removed_capture, &logs[side]);
		results[side] = queries[side](present ? source : NULL);
		set_log(previous, param);
		equal = equal && memcmp(before, source, sizeof(*source)) == 0;
		free(source);
		free(before);
	}
	equal = equal && results[0] == results[1] && logs[0].count == logs[1].count &&
		logs[1].count == (present ? 0u : 1u) && logs[0].level == logs[1].level &&
		strcmp(logs[0].message, logs[1].message) == 0;
	*answer = results[1];
	return equal;
}
