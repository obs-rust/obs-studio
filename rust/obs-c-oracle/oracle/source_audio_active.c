/* Original C query, real-header field adapter, and callback-only log probe.
 * No intentional differences or excluded defined inputs. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_audio_active oracle_obs_source_audio_active
#include "obs-source-audio-active.c.h"
#undef obs_source_audio_active
#include "source_audio_active_fields.inc"

struct source_audio_active_fixture {
	struct obs_source source;
	unsigned char snapshot[sizeof(struct obs_source)];
};
void *oracle_source_audio_active_new(unsigned bits);
void *oracle_source_audio_active_new(unsigned bits)
{
	struct source_audio_active_fixture *f = calloc(1, sizeof(*f));
	if (f) {
		f->source.audio_active = (bits & 1) != 0;
		f->source.audio_pending = (bits & 2) != 0;
		f->source.enabled = (bits & 4) != 0;
		f->source.removed = (bits & 8) != 0;
		memcpy(f->snapshot, &f->source, sizeof(f->source));
	}
	return f;
}
bool oracle_source_audio_active_unchanged(const void *source);
bool oracle_source_audio_active_unchanged(const void *source)
{
	const struct source_audio_active_fixture *f = source;
	return memcmp(&f->source, f->snapshot, sizeof(f->source)) == 0;
}
void oracle_source_audio_active_free(void *source);
void oracle_source_audio_active_free(void *source)
{
	free(source);
}
void oracle_source_audio_active_layout(size_t *layout);
void oracle_source_audio_active_layout(size_t *layout)
{
	layout[0] = sizeof(bool);
	layout[1] = _Alignof(bool);
}
struct source_audio_active_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_audio_active_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_audio_active_capture(int level, const char *format, va_list args, void *param)
{
	struct source_audio_active_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
typedef bool (*source_audio_active_getter)(const obs_source_t *);
bool oracle_source_audio_active_null_log(source_audio_active_getter getter);
bool oracle_source_audio_active_null_log(source_audio_active_getter getter)
{
	struct source_audio_active_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(source_audio_active_capture, &log);
	bool result = getter(NULL);
	base_set_log_handler(old_handler, old_param);
	return !result && log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_source_audio_active: Null 'source' parameter") == 0;
}
