/* Original obs_source_active() and obs_source_showing() over the real private
 * layout, and the production counter adapters the Rust shims read through
 * (obs-source-showing-active-fields.c.h, defined once, here). */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_active oracle_obs_source_active
#define obs_source_showing oracle_obs_source_showing
#include "libobs/build/libobs.pch.h"
#include "source-showing-active.inc"
#undef obs_source_showing
#undef obs_source_active
#include "source_showing_active_fields.inc"

struct source_showing_active_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_showing_active_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_showing_active_capture(int level, const char *format, va_list args, void *param)
{
	struct source_showing_active_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef bool (*source_showing_active_query)(const obs_source_t *);
typedef void (*source_showing_active_get_log)(log_handler_t *, void **);
typedef void (*source_showing_active_set_log)(log_handler_t, void *);

/* One source, or NULL when present is false: every byte is noise, then
 * activate_refs and show_refs take the given values. The Rust pair and the
 * original pair each answer both predicates; each side must answer alike,
 * log alike (the predicate's own name once each for NULL, never otherwise)
 * and leave every byte unchanged. The shared answers go to *active and
 * *showing. */
bool oracle_source_showing_active_check(source_showing_active_query active_proc,
					source_showing_active_query showing_proc, source_showing_active_get_log get_log,
					source_showing_active_set_log set_log, bool present, uint8_t noise,
					long activate_refs, long show_refs, bool *active, bool *showing);
bool oracle_source_showing_active_check(source_showing_active_query active_proc,
					source_showing_active_query showing_proc, source_showing_active_get_log get_log,
					source_showing_active_set_log set_log, bool present, uint8_t noise,
					long activate_refs, long show_refs, bool *active, bool *showing)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	source_showing_active_query queries[2][2] = {{active_proc, showing_proc},
						     {oracle_obs_source_active, oracle_obs_source_showing}};
	bool results[2][2];
	struct source_showing_active_log logs[2][2];
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
		source->activate_refs = activate_refs;
		source->show_refs = show_refs;
		memcpy(before, source, sizeof(*source));
		for (size_t q = 0; q < 2; q++) {
			memset(&logs[side][q], 0, sizeof(logs[side][q]));
			logs[side][q].level = -1;
			set_log(source_showing_active_capture, &logs[side][q]);
			results[side][q] = queries[side][q](present ? source : NULL);
			set_log(previous, param);
		}
		equal = equal && memcmp(before, source, sizeof(*source)) == 0;
		free(source);
		free(before);
	}
	for (size_t q = 0; q < 2; q++) {
		equal = equal && results[0][q] == results[1][q] && logs[0][q].count == logs[1][q].count &&
			logs[1][q].count == (present ? 0u : 1u) && logs[0][q].level == logs[1][q].level &&
			strcmp(logs[0][q].message, logs[1][q].message) == 0;
	}
	*active = results[1][0];
	*showing = results[1][1];
	return equal;
}
