/* Tier 3: the unchanged extracted obs_source_remove()
 * (libobs/obs-source-remove.c.h) over the real private layout, and the
 * production field adapters the Rust shim calls
 * (obs-source-remove-fields.c.h, defined once, here).
 *
 * obs_source_get_ref(), obs_source_dosignal() (obs-internal.h),
 * obs_canvas_remove_source() and obs_source_release() are recording
 * stand-ins for both sides; the stand-in signal keeps obs_source_dosignal()'s
 * rule (the global signal only for a source that is not private, read when
 * it signals). Each call is recorded in order with the source it names and
 * that source's removed flag at that moment. The stand-in reference can be
 * NULL, the source itself or a second source (the twin), so the record
 * shows which pointer each read and step uses. The real calls run through
 * the real core in test_source_remove. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_remove oracle_obs_source_remove
#include "libobs/build/libobs.pch.h"

#define SR_EVENTS 8
/* One event: kind, source id, its removed flag, and the signal name check. */
#define SR_EVENT_WIDTH 4

/* Source ids. */
enum {
	SR_NULL = -1,
	SR_OTHER = -2,
	SR_SOURCE = 0,
	SR_TWIN = 1,
};

/* Event kinds. */
enum {
	SR_EV_GET_REF = 1,
	SR_EV_GLOBAL_SIGNAL,
	SR_EV_SOURCE_SIGNAL,
	SR_EV_CANVAS_REMOVE,
	SR_EV_RELEASE,
	SR_EV_LOG,
};

/* What the stand-in obs_source_get_ref() returns. */
enum {
	SR_REF_NULL = 0,
	SR_REF_SELF,
	SR_REF_TWIN,
};

struct sr_fixture {
	struct obs_source sources[2];
	struct obs_source before[2];
	int reference;
	long events[SR_EVENTS][SR_EVENT_WIDTH];
	size_t event_count;
	bool overflow;
	char message[128];
};

/* Per thread: the Rust test harness may run cases on several threads. */
static _Thread_local struct sr_fixture *sr_current;
/* What a set canvas pointer points at; never followed. */
static char sr_canvas_marker;

static long sr_id(const void *source)
{
	struct sr_fixture *f = sr_current;
	if (!source)
		return SR_NULL;
	for (long i = 0; i < 2; i++) {
		if (source == &f->sources[i])
			return i;
	}
	return SR_OTHER;
}

static void sr_event(long kind, const obs_source_t *source, long extra)
{
	struct sr_fixture *f = sr_current;
	if (f->event_count >= SR_EVENTS) {
		f->overflow = true;
		return;
	}
	long id = sr_id(source);
	long *e = f->events[f->event_count++];
	e[0] = kind;
	e[1] = id;
	e[2] = id >= 0 ? (long)os_atomic_load_bool(&f->sources[id].removed) : -1;
	e[3] = extra;
}

static obs_source_t *sr_get_ref(obs_source_t *source)
{
	struct sr_fixture *f = sr_current;
	sr_event(SR_EV_GET_REF, source, 0);
	switch (f->reference) {
	case SR_REF_SELF:
		return source;
	case SR_REF_TWIN:
		return &f->sources[SR_TWIN];
	default:
		return NULL;
	}
}

static void sr_dosignal(struct obs_source *source, const char *signal_obs, const char *signal_source)
{
	if (signal_obs && !source->context.private)
		sr_event(SR_EV_GLOBAL_SIGNAL, source, strcmp(signal_obs, "source_remove") == 0);
	if (signal_source)
		sr_event(SR_EV_SOURCE_SIGNAL, source, strcmp(signal_source, "remove") == 0);
}

static void sr_canvas_remove(obs_source_t *source)
{
	sr_event(SR_EV_CANVAS_REMOVE, source, 0);
}

static void sr_release(obs_source_t *source)
{
	sr_event(SR_EV_RELEASE, source, 0);
}

#define obs_source_get_ref sr_get_ref
#define obs_source_dosignal sr_dosignal
#define obs_canvas_remove_source sr_canvas_remove
#define obs_source_release sr_release
#include "source-remove.inc"
#undef obs_source_remove
#include "source_remove_fields.inc"
#undef obs_source_release
#undef obs_canvas_remove_source
#undef obs_source_dosignal
#undef obs_source_get_ref

/* Whether before and after differ only inside `removed`. */
static bool sr_unchanged(const struct obs_source *before, const struct obs_source *after)
{
	const unsigned char *b = (const unsigned char *)before;
	const unsigned char *a = (const unsigned char *)after;
	size_t at = offsetof(struct obs_source, removed);
	for (size_t i = 0; i < sizeof(*before); i++) {
		if (i >= at && i < at + sizeof(before->removed))
			continue;
		if (a[i] != b[i])
			return false;
	}
	return true;
}

struct sr_log {
	unsigned count;
};
#if defined(__GNUC__) || defined(__clang__)
static void sr_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void sr_capture(int level, const char *format, va_list args, void *param)
{
	struct sr_log *captured = param;
	captured->count++;
	vsnprintf(sr_current->message, sizeof(sr_current->message), format, args);
	sr_event(SR_EV_LOG, NULL, level);
}

static void sr_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		len = size - 1;
	memcpy(dst, src, len);
	dst[len] = '\0';
}

typedef void (*sr_remove)(obs_source_t *);
typedef void (*sr_get_log)(log_handler_t *, void **);
typedef void (*sr_set_log)(log_handler_t, void *);

/* The out array, by index. */
enum {
	SR_OUT_REMOVED = 0,
	SR_OUT_TWIN_REMOVED,
	SR_OUT_UNCHANGED,
	SR_OUT_LOGS,
	SR_OUT_EVENTS,
	SR_OUT_OVERFLOW,
	SR_OUT_HEAD,
	SR_OUT_COUNT = SR_OUT_HEAD + SR_EVENTS * SR_EVENT_WIDTH,
};

/* Runs obs_source_remove() once through `remove` (the Rust export or a
 * mutant; NULL for the original) on a fresh noise-filled fixture: the source
 * (NULL when present is false) and its twin. Bits of `flags`: 0 the source
 * is removed, 1 it has a canvas, 2 it is private, 3 the twin has a canvas,
 * 4 the twin is private. `reference` is what obs_source_get_ref() returns
 * (SR_REF_*). Writes SR_OUT_COUNT values to out and the last log line to
 * message; false when the fixture could not be built. */
bool oracle_sr_run(sr_remove remove, sr_get_log get_log, sr_set_log set_log, bool present, uint8_t noise,
		   unsigned flags, int reference, long *out, char *message, size_t message_size);
bool oracle_sr_run(sr_remove remove, sr_get_log get_log, sr_set_log set_log, bool present, uint8_t noise,
		   unsigned flags, int reference, long *out, char *message, size_t message_size)
{
	if (message_size == 0)
		return false;
	struct sr_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	memset(f, noise, sizeof(*f));
	f->sources[SR_SOURCE].removed = (flags & 1) != 0;
	f->sources[SR_SOURCE].canvas = (flags & 2) ? (obs_weak_canvas_t *)&sr_canvas_marker : NULL;
	f->sources[SR_SOURCE].context.private = (flags & 4) != 0;
	f->sources[SR_TWIN].removed = false;
	f->sources[SR_TWIN].canvas = (flags & 8) ? (obs_weak_canvas_t *)&sr_canvas_marker : NULL;
	f->sources[SR_TWIN].context.private = (flags & 16) != 0;
	f->reference = reference;
	f->event_count = 0;
	f->overflow = false;
	f->message[0] = '\0';
	memcpy(f->before, f->sources, sizeof(f->sources));

	obs_source_t *source = present ? &f->sources[SR_SOURCE] : NULL;
	struct sr_log captured = {0};
	log_handler_t previous;
	void *param;

	sr_current = f;
	get_log(&previous, &param);
	set_log(sr_capture, &captured);
	if (remove)
		remove(source);
	else
		oracle_obs_source_remove(source);
	set_log(previous, param);

	memset(out, 0, sizeof(long) * SR_OUT_COUNT);
	out[SR_OUT_REMOVED] = os_atomic_load_bool(&f->sources[SR_SOURCE].removed);
	out[SR_OUT_TWIN_REMOVED] = os_atomic_load_bool(&f->sources[SR_TWIN].removed);
	out[SR_OUT_UNCHANGED] = sr_unchanged(&f->before[SR_SOURCE], &f->sources[SR_SOURCE]) &&
				sr_unchanged(&f->before[SR_TWIN], &f->sources[SR_TWIN]);
	out[SR_OUT_LOGS] = (long)captured.count;
	out[SR_OUT_EVENTS] = (long)f->event_count;
	out[SR_OUT_OVERFLOW] = f->overflow;
	for (size_t i = 0; i < f->event_count; i++)
		memcpy(&out[SR_OUT_HEAD + i * SR_EVENT_WIDTH], f->events[i], sizeof(f->events[i]));
	sr_copy(message, message_size, f->message);

	sr_current = NULL;
	free(f);
	return true;
}

/* The sizes the Rust side mirrors: 0 the out array, 1 the event width. */
size_t oracle_sr_layout(size_t which);
size_t oracle_sr_layout(size_t which)
{
	return which == 0 ? SR_OUT_COUNT : which == 1 ? SR_EVENT_WIDTH : 0;
}
