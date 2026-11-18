/* Original getter, original audio predicate, and real private-layout fixtures. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_source_get_audio_mixers oracle_obs_source_get_audio_mixers
#define obs_source_monitoring_has_audio oracle_obs_source_monitoring_has_audio
#include "obs-source-audio-mixers-getter.c.h"
#undef obs_source_monitoring_has_audio
#undef obs_source_get_audio_mixers
#include "source_audio_mixers_fields.inc"

struct source_audio_mixers_fixture {
	struct obs_source source;
	struct obs_source before;
};

void *oracle_source_audio_mixers_new(bool owned, uint32_t flags, uint32_t mixers);
void *oracle_source_audio_mixers_new(bool owned, uint32_t flags, uint32_t mixers)
{
	struct source_audio_mixers_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.owns_info_id = owned;
		fixture->source.info.output_flags = flags;
		fixture->source.audio_mixers = mixers;
		memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

bool oracle_source_audio_mixers_unchanged(const void *handle);
bool oracle_source_audio_mixers_unchanged(const void *handle)
{
	const struct source_audio_mixers_fixture *fixture = handle;
	return memcmp(&fixture->source, &fixture->before, sizeof(fixture->source)) == 0;
}

bool oracle_source_audio_mixers_fields(const obs_source_t *source);
bool oracle_source_audio_mixers_fields(const obs_source_t *source)
{
	return obs_source_audio_mixers_owned_field(source) == source->owns_info_id &&
	       obs_source_audio_mixers_flags_field(source) == source->info.output_flags &&
	       obs_source_audio_mixers_mask_field(source) == source->audio_mixers;
}

void oracle_source_audio_mixers_layout(size_t *layout);
void oracle_source_audio_mixers_layout(size_t *layout)
{
	layout[0] = sizeof(bool);
	layout[1] = _Alignof(bool);
	layout[2] = sizeof(uint32_t);
	layout[3] = _Alignof(uint32_t);
	layout[4] = OBS_SOURCE_AUDIO;
}

void oracle_source_audio_mixers_free(void *handle);
void oracle_source_audio_mixers_free(void *handle)
{
	free(handle);
}

struct source_audio_mixers_log {
	unsigned count;
	int level;
	char message[256];
};
/* Same format annotation as source_handler_getters.c: Clang checks the
 * vsnprintf forwarding call under -Wformat-nonliteral. */
#if defined(__GNUC__) || defined(__clang__)
static void source_audio_mixers_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_audio_mixers_capture(int level, const char *format, va_list args, void *param)
{
	struct source_audio_mixers_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef uint32_t (*source_audio_mixers_getter)(const obs_source_t *);
typedef void (*source_audio_mixers_get_log)(log_handler_t *, void **);
typedef void (*source_audio_mixers_set_log)(log_handler_t, void *);
bool oracle_source_audio_mixers_logs(source_audio_mixers_getter getter, source_audio_mixers_get_log get_log,
				     source_audio_mixers_set_log set_log);
bool oracle_source_audio_mixers_logs(source_audio_mixers_getter getter, source_audio_mixers_get_log get_log,
				     source_audio_mixers_set_log set_log)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	struct obs_source source = {0};
	source.audio_mixers = UINT32_MAX;
	const obs_source_t *inputs[] = {NULL, &source};
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		struct source_audio_mixers_log r = {0}, c = {0};
		set_log(source_audio_mixers_capture, &r);
		uint32_t rust_value = getter(inputs[i]);
		set_log(source_audio_mixers_capture, &c);
		uint32_t c_value = oracle_obs_source_get_audio_mixers(inputs[i]);
		equal = equal && rust_value == c_value && rust_value == 0 && r.count == c.count && r.level == c.level &&
			strcmp(r.message, c.message) == 0;
		if (i == 0)
			equal = equal && r.count == 1 && r.level == LOG_DEBUG &&
				strcmp(r.message, "obs_source_get_audio_mixers: Null 'source' parameter") == 0;
		else
			equal = equal && r.count == 0;
	}
	set_log(previous, param);
	return equal;
}
