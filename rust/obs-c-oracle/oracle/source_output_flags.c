/* Original output flags query over the real private layout. The flags field
 * adapter the Rust shim reads (obs_source_audio_mixers_flags_field) is
 * defined once, by source_audio_mixers_getter.c, in this same oracle
 * library. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_output_flags oracle_obs_source_get_output_flags
#include "libobs/build/libobs.pch.h"
#include "source-output-flags.inc"
#undef obs_source_get_output_flags

struct source_output_flags_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_output_flags_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_output_flags_capture(int level, const char *format, va_list args, void *param)
{
	struct source_output_flags_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef uint32_t (*source_output_flags_query)(const obs_source_t *);
typedef void (*source_output_flags_get_log)(log_handler_t *, void **);
typedef void (*source_output_flags_set_log)(log_handler_t, void *);

/* One source, or NULL when present is false: every byte is noise, then
 * info.output_flags, the instance flags and default_flags take the given
 * values. Both implementations must return alike, log alike (once for NULL,
 * never otherwise) and leave every byte unchanged. Returns the shared
 * result in *result. */
bool oracle_source_output_flags_check(source_output_flags_query proc, source_output_flags_get_log get_log,
				      source_output_flags_set_log set_log, bool present, uint8_t noise,
				      uint32_t info_flags, uint32_t instance_flags, uint32_t default_flags,
				      uint32_t *result);
bool oracle_source_output_flags_check(source_output_flags_query proc, source_output_flags_get_log get_log,
				      source_output_flags_set_log set_log, bool present, uint8_t noise,
				      uint32_t info_flags, uint32_t instance_flags, uint32_t default_flags,
				      uint32_t *result)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	source_output_flags_query queries[] = {proc, oracle_obs_source_get_output_flags};
	uint32_t results[2];
	struct source_output_flags_log logs[2];
	bool equal = true;
	for (size_t q = 0; q < 2; q++) {
		struct obs_source *source = malloc(sizeof(*source));
		struct obs_source *before = malloc(sizeof(*before));
		if (!source || !before) {
			free(source);
			free(before);
			return false;
		}
		memset(source, noise, sizeof(*source));
		source->info.output_flags = info_flags;
		source->flags = instance_flags;
		source->default_flags = default_flags;
		memcpy(before, source, sizeof(*source));
		memset(&logs[q], 0, sizeof(logs[q]));
		set_log(source_output_flags_capture, &logs[q]);
		results[q] = queries[q](present ? source : NULL);
		set_log(previous, param);
		equal = equal && memcmp(before, source, sizeof(*source)) == 0;
		free(source);
		free(before);
	}
	equal = equal && results[0] == results[1] && logs[0].count == logs[1].count &&
		logs[0].count == (present ? 0u : 1u) && logs[0].level == logs[1].level &&
		strcmp(logs[0].message, logs[1].message) == 0;
	*result = results[1];
	return equal;
}
