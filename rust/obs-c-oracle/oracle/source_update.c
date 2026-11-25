/* Tier 3: the original extracted obs_source_update() and the production field
 * adapter over the real private source layout, with a recording update
 * callback, real source and core signal handlers and an optional reentrant
 * nested update. Both sides merge through the same obs_data_apply().
 * A private core keeps this fixture from sharing another oracle's global;
 * Rust runs the log probe in an isolated child. */
#define obs source_update_oracle_core
#include "obs-internal.h"
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

struct obs_core *obs;
void oracle_obs_source_update(obs_source_t *source, obs_data_t *settings);
#define obs_source_update oracle_obs_source_update
#include "obs-source-update.c.h"
#undef obs_source_update
#include "source_update_fields.inc"

typedef void (*source_update_fn)(obs_source_t *, obs_data_t *);

struct source_update_fixture {
	struct obs_core core;
	struct obs_source source;
	int plugin_data;
	source_update_fn entry;
	obs_data_t *nested;
	bool reenter;
	bool inside;
	bool identity;
	unsigned updates;
	char events[64];
	size_t len;
	char *seen;
};

static void source_update_event(struct source_update_fixture *f, char event)
{
	if (f->len + 1 < sizeof(f->events))
		f->events[f->len++] = event;
}

static void source_update_record(void *data, obs_data_t *settings)
{
	struct source_update_fixture *f =
		(struct source_update_fixture *)((char *)data - offsetof(struct source_update_fixture, plugin_data));
	f->updates++;
	f->identity = f->identity && data == &f->plugin_data && settings == f->source.context.settings;
	source_update_event(f, 'U');
	free(f->seen);
	const char *json = settings ? obs_data_get_json(settings) : NULL;
	f->seen = json ? strdup(json) : NULL;
	if (f->reenter && !f->inside) {
		f->inside = true;
		f->entry(&f->source, f->nested);
		f->inside = false;
		source_update_event(f, ')');
	}
}

static void source_update_local(void *param, calldata_t *data)
{
	struct source_update_fixture *f = param;
	f->identity = f->identity && calldata_ptr(data, "source") == &f->source;
	source_update_event(f, 'L');
}

static void source_update_global(void *param, const char *name, calldata_t *data)
{
	struct source_update_fixture *f = param;
	f->identity = f->identity && strcmp(name, "source_update") == 0 && calldata_ptr(data, "source") == &f->source;
	source_update_event(f, 'G');
}

/* bits: 0 has data, 1 has update, 2 private, 3 reenter. */
void *oracle_source_update_new(source_update_fn entry, obs_data_t *stored, obs_data_t *nested, uint32_t flags,
			       uint32_t bits, long defer);
void *oracle_source_update_new(source_update_fn entry, obs_data_t *stored, obs_data_t *nested, uint32_t flags,
			       uint32_t bits, long defer)
{
	struct source_update_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->entry = entry;
	f->nested = nested;
	f->identity = true;
	f->reenter = (bits & 8u) != 0;
	f->source.context.settings = stored;
	f->source.context.data = (bits & 1u) ? &f->plugin_data : NULL;
	f->source.context.private = (bits & 4u) != 0;
	f->source.info.update = (bits & 2u) ? source_update_record : NULL;
	f->source.info.output_flags = flags;
	f->source.defer_update_count = defer;
	f->source.context.signals = signal_handler_create();
	f->core.signals = signal_handler_create();
	signal_handler_add(f->source.context.signals, "void update(ptr source)");
	signal_handler_add(f->core.signals, "void source_update(ptr source)");
	signal_handler_connect(f->source.context.signals, "update", source_update_local, f);
	signal_handler_connect_global(f->core.signals, source_update_global, f);
	return f;
}

/* Run one update on the fixture's source (or NULL) under its private core. */
void oracle_source_update_run(void *handle, bool null_source, obs_data_t *incoming);
void oracle_source_update_run(void *handle, bool null_source, obs_data_t *incoming)
{
	struct source_update_fixture *f = handle;
	struct obs_core *old = obs;
	obs = &f->core;
	f->entry(null_source ? NULL : &f->source, incoming);
	obs = old;
}

/* The record: events ("U" update, "G" core signal, "L" source signal, ")"
 * end of a nested update), update count, identity of every callback's
 * arguments, the settings the last update saw, the defer count, and whether
 * the source still holds its settings object, data and callback. */
const char *oracle_source_update_record(const void *handle, unsigned *updates, bool *identity, const char **seen,
					long *defer, obs_data_t *stored, uint32_t bits);
const char *oracle_source_update_record(const void *handle, unsigned *updates, bool *identity, const char **seen,
					long *defer, obs_data_t *stored, uint32_t bits)
{
	const struct source_update_fixture *f = handle;
	*updates = f->updates;
	*seen = f->seen;
	*defer = f->source.defer_update_count;
	*identity = f->identity && f->source.context.settings == stored &&
		    f->source.context.data == ((bits & 1u) ? &f->plugin_data : NULL) &&
		    (f->source.info.update != NULL) == ((bits & 2u) != 0);
	return f->events;
}

void oracle_source_update_free(void *handle);
void oracle_source_update_free(void *handle)
{
	struct source_update_fixture *f = handle;
	if (!f)
		return;
	signal_handler_destroy(f->source.context.signals);
	signal_handler_destroy(f->core.signals);
	free(f->seen);
	free(f);
}

struct source_update_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_update_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_update_capture(int level, const char *format, va_list args, void *param)
{
	struct source_update_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*source_update_get_log)(log_handler_t *, void **);
typedef void (*source_update_set_log)(log_handler_t, void *);
bool oracle_source_update_logs(source_update_fn update, source_update_get_log get_log, source_update_set_log set_log,
			       obs_data_t *incoming);
bool oracle_source_update_logs(source_update_fn update, source_update_get_log get_log, source_update_set_log set_log,
			       obs_data_t *incoming)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct source_update_log r = {0}, c = {0};
	set_log(source_update_capture, &r);
	update(NULL, incoming);
	set_log(source_update_capture, &c);
	oracle_obs_source_update(NULL, incoming);
	set_log(old_handler, old_param);
	return r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_source_update: Null 'source' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
#undef obs
