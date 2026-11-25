/* Tier 3: the unchanged extracted obs_source_set_flags() and
 * obs_source_set_audio_mixers(), and the production adapters the Rust shims
 * read through (obs-source-flags-mixers-fields.c.h, defined once, here), over
 * the real private layout with a real signal handler. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_set_flags oracle_obs_source_set_flags
#define obs_source_set_audio_mixers oracle_obs_source_set_audio_mixers
#include "libobs/build/libobs.pch.h"
#include "source-flags-mixers.inc"
#undef obs_source_set_audio_mixers
#undef obs_source_set_flags
#include "source_flags_mixers_fields.inc"

typedef void (*flags_mixers_setter)(obs_source_t *, uint32_t);

#define FLAGS_MIXERS_OPS 32
#define FLAGS_MIXERS_EVENTS 4
/* Per operation: the event count, three words per event, the two fields
 * after, and whether every other byte is as before. */
#define FLAGS_MIXERS_TRACE (1 + 3 * FLAGS_MIXERS_EVENTS + 2)

/* What the audio_mixers handler does: leave the value, rewrite it as an
 * int, rewrite it as a bool (another size), or set the mixers again from
 * inside (the update_flags handler sets the flags again instead). */
enum { FLAGS_MIXERS_LEAVE, FLAGS_MIXERS_REWRITE, FLAGS_MIXERS_RESIZE, FLAGS_MIXERS_REENTER, FLAGS_MIXERS_MODES };

struct flags_mixers_fixture {
	struct obs_source source;
	struct obs_source before;
	flags_mixers_setter set_flags;
	flags_mixers_setter set_mixers;
	int mode;
	long long rewrite;
	bool inside;
	size_t events;
	uint64_t trace[3 * FLAGS_MIXERS_EVENTS];
};

/* One event: which signal, whether the calldata names the fixture's source,
 * whether it is the fixed 128-byte stack, its size; the value it carries;
 * and the field the setter writes, as the handler sees it. */
static void flags_mixers_record(struct flags_mixers_fixture *f, calldata_t *data, uint64_t signal, const char *name,
				uint32_t field)
{
	if (f->events < FLAGS_MIXERS_EVENTS) {
		uint64_t *e = &f->trace[3 * f->events];
		e[0] = signal | (calldata_ptr(data, "source") == &f->source ? 2u : 0u) |
		       (data->fixed && data->capacity == 128 ? 4u : 0u) | ((uint64_t)data->size << 8);
		e[1] = (uint64_t)calldata_int(data, name);
		e[2] = field;
	}
	f->events++;
}

static void flags_mixers_on_flags(void *param, calldata_t *data)
{
	struct flags_mixers_fixture *f = param;
	flags_mixers_record(f, data, 0, "flags", f->source.flags);
	if (f->mode == FLAGS_MIXERS_REENTER && !f->inside) {
		f->inside = true;
		f->set_flags(&f->source, f->source.flags ^ 0x10u);
		f->inside = false;
	}
}

static void flags_mixers_on_mixers(void *param, calldata_t *data)
{
	struct flags_mixers_fixture *f = param;
	flags_mixers_record(f, data, 1, "mixers", f->source.audio_mixers);
	switch (f->mode) {
	case FLAGS_MIXERS_REWRITE:
		calldata_set_int(data, "mixers", f->rewrite);
		break;
	case FLAGS_MIXERS_RESIZE:
		calldata_set_bool(data, "mixers", true);
		break;
	case FLAGS_MIXERS_REENTER:
		if (!f->inside) {
			f->inside = true;
			f->set_mixers(&f->source, (uint32_t)(uint64_t)f->rewrite);
			f->inside = false;
		}
		break;
	default:
		break;
	}
}

/* Runs count operations on a fixture filled with noise: kinds[i] 0 sets the
 * flags, 1 the mixers, to values[i], with the given setters. Writes
 * FLAGS_MIXERS_TRACE words per operation to trace. False when the fixture
 * could not be built, an adapter does not reach its field, or a setter wrote
 * a byte other than the two fields. */
bool oracle_source_flags_mixers_run(flags_mixers_setter set_flags, flags_mixers_setter set_mixers, uint8_t noise,
				    uint32_t output_flags, bool owns_info_id, int mode, long long rewrite,
				    const uint8_t *kinds, const uint32_t *values, size_t count, uint64_t *trace);
bool oracle_source_flags_mixers_run(flags_mixers_setter set_flags, flags_mixers_setter set_mixers, uint8_t noise,
				    uint32_t output_flags, bool owns_info_id, int mode, long long rewrite,
				    const uint8_t *kinds, const uint32_t *values, size_t count, uint64_t *trace)
{
	if (count > FLAGS_MIXERS_OPS || mode < 0 || mode >= FLAGS_MIXERS_MODES)
		return false;
	struct flags_mixers_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	memset(f, noise, sizeof(*f));
	f->source.info.output_flags = output_flags;
	f->source.owns_info_id = owns_info_id;
	f->source.context.signals = signal_handler_create();
	if (!f->source.context.signals) {
		free(f);
		return false;
	}
	f->set_flags = set_flags;
	f->set_mixers = set_mixers;
	f->mode = mode;
	f->rewrite = rewrite;
	f->inside = false;
	bool valid = signal_handler_add(f->source.context.signals, "void update_flags(ptr source, int flags)") &&
		     signal_handler_add(f->source.context.signals, "void audio_mixers(ptr source, in out int mixers)");
	signal_handler_connect(f->source.context.signals, "update_flags", flags_mixers_on_flags, f);
	signal_handler_connect(f->source.context.signals, "audio_mixers", flags_mixers_on_mixers, f);

	/* The adapters reach the fields they name. */
	const uint32_t flags = f->source.flags;
	const uint32_t mixers = f->source.audio_mixers;
	valid = valid && obs_source_flags_mixers_flags(&f->source) == flags &&
		obs_source_flags_mixers_audio_mixers(&f->source) == mixers &&
		obs_source_flags_mixers_owns_info_id(&f->source) == owns_info_id &&
		obs_source_flags_mixers_output_flags(&f->source) == output_flags &&
		obs_source_flags_mixers_signals(&f->source) == f->source.context.signals;
	obs_source_flags_mixers_store_flags(&f->source, ~flags);
	obs_source_flags_mixers_store_audio_mixers(&f->source, ~mixers);
	valid = valid && f->source.flags == ~flags && f->source.audio_mixers == ~mixers;
	f->source.flags = flags;
	f->source.audio_mixers = mixers;

	memcpy(&f->before, &f->source, sizeof(f->source));
	for (size_t i = 0; i < count; i++) {
		f->events = 0;
		memset(f->trace, 0, sizeof(f->trace));
		(kinds[i] ? set_mixers : set_flags)(&f->source, values[i]);
		uint64_t *t = &trace[i * FLAGS_MIXERS_TRACE];
		t[0] = f->events;
		memcpy(&t[1], f->trace, sizeof(f->trace));
		t[1 + 3 * FLAGS_MIXERS_EVENTS] = ((uint64_t)f->source.flags << 32) | f->source.audio_mixers;
		f->before.flags = f->source.flags;
		f->before.audio_mixers = f->source.audio_mixers;
		const bool others = memcmp(&f->before, &f->source, sizeof(f->source)) == 0;
		t[2 + 3 * FLAGS_MIXERS_EVENTS] = others;
		valid = valid && others;
	}
	signal_handler_destroy(f->source.context.signals);
	free(f);
	return valid;
}

/* The original setters, for the parity test's other side. */
void oracle_source_flags_mixers_originals(flags_mixers_setter *set_flags, flags_mixers_setter *set_mixers);
void oracle_source_flags_mixers_originals(flags_mixers_setter *set_flags, flags_mixers_setter *set_mixers)
{
	*set_flags = oracle_obs_source_set_flags;
	*set_mixers = oracle_obs_source_set_audio_mixers;
}

struct flags_mixers_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void flags_mixers_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void flags_mixers_capture(int level, const char *format, va_list args, void *param)
{
	struct flags_mixers_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*flags_mixers_get_log)(log_handler_t *, void **);
typedef void (*flags_mixers_set_log)(log_handler_t, void *);

/* A NULL source logs the same single LOG_DEBUG line from the given setters
 * as from the originals. */
bool oracle_source_flags_mixers_null_logs(flags_mixers_setter set_flags, flags_mixers_setter set_mixers,
					  flags_mixers_get_log get, flags_mixers_set_log set);
bool oracle_source_flags_mixers_null_logs(flags_mixers_setter set_flags, flags_mixers_setter set_mixers,
					  flags_mixers_get_log get, flags_mixers_set_log set)
{
	static const char *const expected[2] = {"obs_source_set_flags: Null 'source' parameter",
						"obs_source_set_audio_mixers: Null 'source' parameter"};
	const flags_mixers_setter given[2] = {set_flags, set_mixers};
	const flags_mixers_setter original[2] = {oracle_obs_source_set_flags, oracle_obs_source_set_audio_mixers};
	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	bool valid = true;
	for (size_t i = 0; i < 2; i++) {
		struct flags_mixers_log r = {0}, c = {0};
		set(flags_mixers_capture, &r);
		given[i](NULL, 1);
		set(flags_mixers_capture, &c);
		original[i](NULL, 1);
		set(old_handler, old_param);
		valid = valid && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
			strcmp(c.message, expected[i]) == 0 && strcmp(r.message, c.message) == 0;
	}
	return valid;
}
