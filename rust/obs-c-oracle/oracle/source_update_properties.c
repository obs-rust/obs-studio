/* Tier 3: original entry point and real adapters, with real signal callbacks.
 * A private core prevents this fixture from sharing another oracle's global.
 * Rust runs this fixture in an isolated category child (also for logging). */
#define obs source_update_properties_oracle_core
#include "obs-internal.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

struct obs_core *obs;
void oracle_obs_source_update_properties(obs_source_t *source);
#define obs_source_update_properties oracle_obs_source_update_properties
#include "obs-source-update-properties.c.h"
#undef obs_source_update_properties
#include "source_update_properties_fields.inc"

typedef void (*update_properties_entry)(obs_source_t *);
struct update_properties_fixture {
	struct obs_core core;
	struct obs_source source;
	unsigned char before[sizeof(struct obs_source)];
	uint64_t settings[4];
	update_properties_entry entry;
	bool mutate;
	bool reenter;
	bool inside;
	unsigned calls;
	unsigned global_calls;
	bool payload_ok;
};

static void update_properties_capture(void *param, calldata_t *data)
{
	struct update_properties_fixture *f = param;
	f->calls++;
	f->payload_ok = f->payload_ok && calldata_ptr(data, "source") == &f->source && data->fixed &&
			data->capacity == 128;
	if (f->mutate)
		calldata_set_ptr(data, "source", NULL);
	if (f->reenter && !f->inside) {
		f->inside = true;
		f->entry(&f->source);
		f->inside = false;
	}
}

static void update_properties_core_capture(void *param, const char *name, calldata_t *data)
{
	(void)name;
	(void)data;
	struct update_properties_fixture *f = param;
	f->global_calls++;
}

bool oracle_source_update_properties_run(update_properties_entry entry, bool private_source, unsigned count,
					 bool reenter, bool mutate, unsigned *trace);
bool oracle_source_update_properties_run(update_properties_entry entry, bool private_source, unsigned count,
					 bool reenter, bool mutate, unsigned *trace)
{
	if (count > 16)
		return false;
	struct update_properties_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return false;
	obs = &f->core;
	f->entry = entry;
	f->mutate = mutate;
	f->reenter = reenter;
	f->payload_ok = true;
	f->settings[0] = UINT64_C(0x123456789abcdef0);
	f->source.context.settings = (void *)f->settings;
	f->source.context.private = private_source;
	f->source.context.signals = signal_handler_create();
	f->core.signals = signal_handler_create();
	bool valid = signal_handler_add(f->source.context.signals, "void update_properties(ptr source)");
	signal_handler_connect(f->source.context.signals, "update_properties", update_properties_capture, f);
	signal_handler_connect_global(f->core.signals, update_properties_core_capture, f);
	memcpy(f->before, &f->source, sizeof(f->source));
	for (unsigned i = 0; i < count; i++) {
		entry(&f->source);
		valid = valid && f->calls == (i + 1) * (reenter ? 2u : 1u) && f->global_calls == 0 && f->payload_ok &&
			memcmp(f->before, &f->source, sizeof(f->source)) == 0 &&
			f->settings[0] == UINT64_C(0x123456789abcdef0) && f->settings[1] == 0 && f->settings[2] == 0 &&
			f->settings[3] == 0;
		trace[i] = f->calls | (f->global_calls << 8) | (f->payload_ok ? 1u << 16 : 0);
	}
	signal_handler_destroy(f->source.context.signals);
	signal_handler_destroy(f->core.signals);
	obs = NULL;
	free(f);
	return valid;
}

struct update_properties_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void update_properties_capture_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void update_properties_capture_log(int level, const char *format, va_list args, void *param)
{
	struct update_properties_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*update_properties_get_log)(log_handler_t *, void **);
typedef void (*update_properties_set_log)(log_handler_t, void *);
bool oracle_source_update_properties_null_logs(update_properties_entry entry, update_properties_get_log get,
					       update_properties_set_log set);
bool oracle_source_update_properties_null_logs(update_properties_entry entry, update_properties_get_log get,
					       update_properties_set_log set)
{
	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	struct update_properties_log r = {0}, c = {0};
	set(update_properties_capture_log, &r);
	entry(NULL);
	set(update_properties_capture_log, &c);
	oracle_obs_source_update_properties(NULL);
	set(old_handler, old_param);
	return r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_source_update_properties: Null 'source' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
#undef obs
