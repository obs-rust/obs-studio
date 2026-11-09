/* Exact extracted C setter, with a recording seam for the existing dosignal
 * primitive. Actual signal handlers/calldata are covered by unchanged Tier 2. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-source-monitoring.h"

struct setter_fixture {
	struct obs_source source;
	unsigned trace;
};

static void setter_record(obs_source_t *source, const char *global, const char *local)
{
	struct setter_fixture *fixture = (struct setter_fixture *)source;
	bool active = os_atomic_load_bool(&source->audio_active);
	unsigned first = strcmp(global, "source_audio_activate") == 0     ? 1
			 : strcmp(global, "source_audio_deactivate") == 0 ? 3
									  : 9;
	bool names_match = strcmp(local, first == 1 ? "audio_activate" : "audio_deactivate") == 0;
	fixture->trace = (source->context.private ? 1u : 2u) << 4;
	fixture->trace |= (first + (source->context.private ? 1u : 0u)) << 8;
	if (!source->context.private)
		fixture->trace |= (first + 1u) << 12;
	if (!names_match || active != (first == 1))
		fixture->trace |= 1u << 20;
}
enum obs_source_monitoring_active oracle_obs_source_monitoring_audio_active(bool previous, bool active);
void oracle_obs_source_set_audio_active(obs_source_t *source, bool active);
#define obs_source_monitoring_audio_active oracle_obs_source_monitoring_audio_active
#define obs_source_dosignal setter_record
#define obs_source_set_audio_active oracle_obs_source_set_audio_active
#include "source_audio_active_setter.inc"
#undef obs_source_set_audio_active
#undef obs_source_monitoring_audio_active
#include "source_audio_active_setter_fields.inc"
#undef obs_source_dosignal

void *oracle_source_audio_active_setter_new(unsigned bits);
void *oracle_source_audio_active_setter_new(unsigned bits)
{
	struct setter_fixture *f = calloc(1, sizeof(*f));
	if (f) {
		f->source.audio_active = (bits & 1) != 0;
		f->source.context.private = (bits & 2) != 0;
		f->source.audio_pending = true;
		f->source.enabled = true;
	}
	return f;
}
void oracle_source_audio_active_setter_free(void *fixture);
void oracle_source_audio_active_setter_free(void *fixture)
{
	free(fixture);
}
unsigned oracle_source_audio_active_setter_step(void *fixture, bool active, void (*setter)(obs_source_t *, bool));
unsigned oracle_source_audio_active_setter_step(void *fixture, bool active, void (*setter)(obs_source_t *, bool))
{
	struct setter_fixture *f = fixture;
	f->trace = 0;
	setter(&f->source, active);
	return f->trace | (unsigned)os_atomic_load_bool(&f->source.audio_active) |
	       ((unsigned)f->source.audio_pending << 1) | ((unsigned)f->source.enabled << 2);
}

struct setter_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void setter_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void setter_capture(int level, const char *format, va_list args, void *param)
{
	struct setter_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_source_audio_active_setter_null_log(void (*setter)(obs_source_t *, bool));
bool oracle_source_audio_active_setter_null_log(void (*setter)(obs_source_t *, bool))
{
	struct setter_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(setter_capture, &log);
	setter(NULL, true);
	setter(NULL, false);
	base_set_log_handler(old_handler, old_param);
	return log.count == 2 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_source_set_audio_active: Null 'source' parameter") == 0;
}
