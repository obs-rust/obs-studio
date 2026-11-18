/* Original configurable query and data_valid macro over the real private layout.
 * The properties field adapter the Rust shim reads is defined once, by
 * source_properties.c, in this same oracle library. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_configurable oracle_obs_source_configurable
#include "libobs/build/libobs.pch.h"
#include "source-configurable.inc"
#undef obs_source_configurable

struct source_configurable_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_configurable_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_configurable_capture(int level, const char *format, va_list args, void *param)
{
	struct source_configurable_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static unsigned source_configurable_calls;
static obs_properties_t *source_configurable_props(void *data)
{
	(void)data;
	source_configurable_calls++;
	return NULL;
}
static obs_properties_t *source_configurable_props2(void *data, void *type_data)
{
	(void)data;
	(void)type_data;
	source_configurable_calls++;
	return NULL;
}

typedef bool (*source_configurable_query)(const obs_source_t *);
typedef void (*source_configurable_get_log)(log_handler_t *, void **);
typedef void (*source_configurable_set_log)(log_handler_t, void *);

/* flags: bit 0 source present, bit 1 data, bit 2 get_properties, bit 3
 * get_properties2. noise fills the unrelated fields, and the type id is never
 * registered, so only the source's own info can answer. Both
 * implementations must answer alike, log alike, call nothing and change
 * nothing. Returns the shared answer in *answer. */
bool oracle_source_configurable_check(source_configurable_query proc, source_configurable_get_log get_log,
				      source_configurable_set_log set_log, unsigned flags, uint32_t noise,
				      bool *answer);
bool oracle_source_configurable_check(source_configurable_query proc, source_configurable_get_log get_log,
				      source_configurable_set_log set_log, unsigned flags, uint32_t noise, bool *answer)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	bool present = (flags & 1) != 0;
	source_configurable_query queries[] = {proc, oracle_obs_source_configurable};
	bool results[2];
	struct source_configurable_log logs[2];
	bool equal = true;
	int data = 0;
	for (size_t q = 0; q < 2; q++) {
		struct obs_source source, before;
		memset(&source, (int)(noise & 0xff), sizeof(source));
		source.context.data = (flags & 2) ? &data : NULL;
		source.info.get_properties = (flags & 4) ? source_configurable_props : NULL;
		source.info.get_properties2 = (flags & 8) ? source_configurable_props2 : NULL;
		source.info.id = "oracle_source_configurable_never_registered";
		source.info.output_flags = noise;
		memcpy(&before, &source, sizeof(source));
		memset(&logs[q], 0, sizeof(logs[q]));
		source_configurable_calls = 0;
		set_log(source_configurable_capture, &logs[q]);
		results[q] = queries[q](present ? &source : NULL);
		set_log(previous, param);
		equal = equal && memcmp(&before, &source, sizeof(source)) == 0 && source_configurable_calls == 0;
	}
	equal = equal && results[0] == results[1] && logs[0].count == logs[1].count &&
		logs[0].count == (present ? 0u : 1u) && logs[0].level == logs[1].level &&
		strcmp(logs[0].message, logs[1].message) == 0;
	*answer = results[1];
	return equal;
}
