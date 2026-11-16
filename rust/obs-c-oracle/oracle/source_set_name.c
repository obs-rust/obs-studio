/* Tier 3: the unchanged extracted source rename and the production adapters,
 * over the real private layouts. Stand-ins trace the canvas rename, the two
 * context setters and signal delivery; the setter's own decisions are not
 * stood in. The actual-library Tier 2 suites cover those dependencies. */
#include "obs-internal.h"
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct source_rename_fixture {
	struct obs_source source; /* first: a context pointer is the fixture */
	struct obs_core core;
	char local_tag;
	char global_tag;
	char *taken;
	struct dstr trace;
	bool flip;
	bool mutate;
};
static struct obs_core *source_rename_obs;
static void source_rename_canvas(obs_source_t *source, const char *name);
static void source_rename_update(struct obs_context_data *context, const char *name);
static void source_rename_update_ht(struct obs_context_data *context, const char *name, void *head);
static void source_rename_signal(signal_handler_t *handler, const char *signal, calldata_t *data);

void oracle_obs_source_set_name(obs_source_t *source, const char *name);
#define obs source_rename_obs
#define obs_canvas_rename_source source_rename_canvas
#define obs_context_data_setname source_rename_update
#define obs_context_data_setname_ht source_rename_update_ht
#define signal_handler_signal source_rename_signal
#define obs_source_set_name oracle_obs_source_set_name
#include "source-set-name.inc"
#undef obs_source_set_name
#include "source_set_name_fields.inc"
#undef signal_handler_signal
#undef obs_context_data_setname_ht
#undef obs_context_data_setname
#undef obs_canvas_rename_source
#undef obs

#if defined(__GNUC__) || defined(__clang__)
static void source_rename_log(struct dstr *trace, const char *format, ...) __attribute__((format(printf, 2, 3)));
#endif
static void source_rename_log(struct dstr *trace, const char *format, ...)
{
	char line[512];
	va_list args;
	va_start(args, format);
	vsnprintf(line, sizeof(line), format, args);
	va_end(args);
	dstr_cat(trace, line);
}

static const char *source_rename_text(const char *name)
{
	return name ? name : "<NULL>";
}

static void source_rename_store(struct source_rename_fixture *f, const char *name)
{
	char *copy = bstrdup(name);
	bfree(f->source.context.name);
	f->source.context.name = copy;
	/* A table setter that changes privacy shows whether the setter reads
	 * it again before choosing its signals. */
	if (f->flip)
		f->source.context.private = !f->source.context.private;
}

static void source_rename_canvas(obs_source_t *source, const char *name)
{
	struct source_rename_fixture *f = (void *)source;
	source_rename_log(&f->trace, "canvas(%s)", source_rename_text(name));
	bfree(f->source.context.name);
	f->source.context.name = bstrdup(name);
}

static void source_rename_update(struct obs_context_data *context, const char *name)
{
	struct source_rename_fixture *f = (void *)context;
	source_rename_log(&f->trace, "unlisted(%s)", source_rename_text(name));
	source_rename_store(f, name);
}

static void source_rename_update_ht(struct obs_context_data *context, const char *name, void *head)
{
	struct source_rename_fixture *f = (void *)context;
	bool table = head == &source_rename_obs->data.public_sources;
	source_rename_log(&f->trace, "public(%s%s)", source_rename_text(name), table ? "" : " wrong table");
	/* One occupied name resolves to a suffix, so new_name is not the request. */
	if (name && strcmp(name, f->taken) == 0) {
		struct dstr resolved = {0};
		dstr_copy(&resolved, name);
		dstr_cat(&resolved, " 2");
		source_rename_store(f, resolved.array);
		dstr_free(&resolved);
	} else {
		source_rename_store(f, name);
	}
}

static void source_rename_signal(signal_handler_t *handler, const char *signal, calldata_t *data)
{
	struct source_rename_fixture *f =
		source_rename_obs ? (void *)((char *)source_rename_obs - offsetof(struct source_rename_fixture, core))
				  : NULL;
	if (!f)
		abort();
	char which = (void *)handler == &f->local_tag ? 'L' : (void *)handler == &f->global_tag ? 'G' : '?';
	source_rename_log(&f->trace, "signal(%c %s source=%d new=%s prev=%s size=%u)", which, signal,
			  calldata_ptr(data, "source") == &f->source,
			  source_rename_text(calldata_string(data, "new_name")),
			  source_rename_text(calldata_string(data, "prev_name")), (unsigned)data->size);
	/* A callback's payload edit is visible to the next signal. */
	if (f->mutate)
		calldata_set_string(data, "prev_name", "callback");
}

void *oracle_source_rename_new(const char *name, const char *taken, uint32_t flags, bool private, bool flip,
			       bool mutate);
void *oracle_source_rename_new(const char *name, const char *taken, uint32_t flags, bool private, bool flip,
			       bool mutate)
{
	struct source_rename_fixture *f = bzalloc(sizeof(*f));
	f->source.context.name = bstrdup(name);
	f->source.context.private = private;
	f->source.context.signals = (void *)&f->local_tag;
	f->source.info.output_flags = flags;
	f->core.signals = (void *)&f->global_tag;
	f->taken = bstrdup(taken);
	f->flip = flip;
	f->mutate = mutate;
	dstr_init_copy(&f->trace, "");
	return f;
}

void oracle_source_rename_select(void *handle);
void oracle_source_rename_select(void *handle)
{
	struct source_rename_fixture *f = handle;
	source_rename_obs = &f->core;
}

/* The trace so far, then cleared; and the source's name and privacy. */
const char *oracle_source_rename_trace(void *handle);
const char *oracle_source_rename_trace(void *handle)
{
	struct source_rename_fixture *f = handle;
	source_rename_log(&f->trace, " -> %s %d", source_rename_text(f->source.context.name),
			  f->source.context.private);
	return f->trace.array;
}

void oracle_source_rename_clear(void *handle);
void oracle_source_rename_clear(void *handle)
{
	struct source_rename_fixture *f = handle;
	dstr_copy(&f->trace, "");
}

void oracle_source_rename_free(void *handle);
void oracle_source_rename_free(void *handle)
{
	struct source_rename_fixture *f = handle;
	bfree(f->source.context.name);
	bfree(f->taken);
	dstr_free(&f->trace);
	bfree(f);
	source_rename_obs = NULL;
}

struct source_rename_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_rename_capture_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_rename_capture_log(int level, const char *format, va_list args, void *param)
{
	struct source_rename_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*source_rename_setter)(obs_source_t *, const char *);
typedef void (*source_rename_get_log)(log_handler_t *, void **);
typedef void (*source_rename_set_log)(log_handler_t, void *);
bool oracle_source_rename_null_logs(source_rename_setter setter, source_rename_get_log get, source_rename_set_log set);
bool oracle_source_rename_null_logs(source_rename_setter setter, source_rename_get_log get, source_rename_set_log set)
{
	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	bool valid = true;
	const char *names[] = {NULL, "", "name"};
	for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); i++) {
		struct source_rename_log r = {0}, c = {0};
		set(source_rename_capture_log, &r);
		setter(NULL, names[i]);
		set(source_rename_capture_log, &c);
		oracle_obs_source_set_name(NULL, names[i]);
		valid = valid && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
			strcmp(r.message, "obs_source_set_name: Null 'source' parameter") == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set(old_handler, old_param);
	return valid;
}
