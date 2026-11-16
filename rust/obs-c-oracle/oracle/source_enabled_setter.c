/* Tier 3: compile the unchanged original setter, not a behavioral stand-in. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_set_enabled oracle_obs_source_set_enabled
#include "obs-source-enabled-setter.c.h"
#undef obs_source_set_enabled
#include "source_enabled_setter_fields.inc"

typedef void (*enabled_setter_fn)(obs_source_t *, bool);
struct enabled_setter_fixture {
	struct obs_source source;
	unsigned char before[sizeof(struct obs_source)];
	enabled_setter_fn setter;
	bool mutate;
	bool reenter;
	bool inside;
	size_t count;
	unsigned events[64];
};

static void enabled_setter_capture(void *param, calldata_t *data)
{
	struct enabled_setter_fixture *f = param;
	bool value = calldata_bool(data, "enabled");
	unsigned event = (calldata_ptr(data, "source") == &f->source ? 1u : 0u) | (value ? 2u : 0u) |
			 (f->source.enabled ? 4u : 0u) | (data->fixed && data->capacity == 128 ? 8u : 0u) |
			 ((unsigned)data->size << 8);
	if (f->count < 64)
		f->events[f->count] = event;
	f->count++;
	if (f->mutate) {
		calldata_set_bool(data, "enabled", !value);
		calldata_set_ptr(data, "source", NULL);
	}
	if (f->reenter && !f->inside) {
		f->inside = true;
		f->setter(&f->source, !value);
		f->inside = false;
	}
}

bool oracle_source_enabled_run(enabled_setter_fn setter, bool initial, bool mutate, bool reenter, const bool *values,
			       size_t count, unsigned *trace);
bool oracle_source_enabled_run(enabled_setter_fn setter, bool initial, bool mutate, bool reenter, const bool *values,
			       size_t count, unsigned *trace)
{
	if (count > 32)
		return false;
	struct enabled_setter_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return false;
	f->source.enabled = initial;
	f->source.context.signals = signal_handler_create();
	f->setter = setter;
	f->mutate = mutate;
	f->reenter = reenter;
	bool valid = signal_handler_add(f->source.context.signals, "void enable(ptr source, bool enabled)");
	signal_handler_connect(f->source.context.signals, "enable", enabled_setter_capture, f);
	memcpy(f->before, &f->source, sizeof(f->source));
	valid = valid && obs_source_enabled_setter_field(&f->source) == &f->source.enabled &&
		obs_source_enabled_setter_signals(&f->source) == f->source.context.signals;
	for (size_t i = 0; i < count; i++) {
		size_t start = f->count;
		setter(&f->source, values[i]);
		unsigned expected = values[i] ? 7u : 1u;
		valid = valid && f->count == start + (reenter ? 2u : 1u) && (f->events[start] & 15u) == (expected | 8u);
		if (reenter)
			valid = valid && (f->events[start + 1] & 15u) == ((values[i] ? 1u : 7u) | 8u);
		valid = valid && f->source.enabled == (reenter ? !values[i] : values[i]);
		trace[i] = f->events[start] | (reenter ? f->events[start + 1] << 4 : 0u) |
			   (f->source.enabled ? 1u << 24 : 0u) | ((unsigned)f->count << 25);
		bool enabled = f->source.enabled;
		f->source.enabled = initial;
		valid = valid && memcmp(f->before, &f->source, sizeof(f->source)) == 0;
		f->source.enabled = enabled;
	}
	signal_handler_destroy(f->source.context.signals);
	free(f);
	return valid;
}

void oracle_source_enabled_layout(size_t *layout);
void oracle_source_enabled_layout(size_t *layout)
{
	layout[0] = sizeof(bool);
	layout[1] = _Alignof(bool);
}

struct enabled_setter_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void enabled_setter_capture_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void enabled_setter_capture_log(int level, const char *format, va_list args, void *param)
{
	struct enabled_setter_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*enabled_setter_get_log)(log_handler_t *, void **);
typedef void (*enabled_setter_set_log)(log_handler_t, void *);
bool oracle_source_enabled_null_logs(enabled_setter_fn setter, enabled_setter_get_log get, enabled_setter_set_log set);
bool oracle_source_enabled_null_logs(enabled_setter_fn setter, enabled_setter_get_log get, enabled_setter_set_log set)
{
	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	bool valid = true;
	for (unsigned i = 0; i < 2; i++) {
		struct enabled_setter_log r = {0}, c = {0};
		set(enabled_setter_capture_log, &r);
		setter(NULL, i != 0);
		set(enabled_setter_capture_log, &c);
		oracle_obs_source_set_enabled(NULL, i != 0);
		valid = valid && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
			strcmp(r.message, "obs_source_set_enabled: Null 'source' parameter") == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set(old_handler, old_param);
	return valid;
}
