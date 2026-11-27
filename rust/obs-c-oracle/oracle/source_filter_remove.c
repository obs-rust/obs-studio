/* Tier 3: the unchanged extracted obs_source_filter_remove() and
 * obs_source_filter_remove_refless() (libobs/obs-source-filter-remove.c.h)
 * over the real private layout, and the production field adapters the Rust
 * shim calls (obs-source-filter-remove-fields.c.h, defined once, here).
 *
 * The original calls the filter chain rule by its libobs name, so it runs on
 * the rule hybrid libobs ships (obs-source's source_filter_chain). libobs's
 * core global is a stand-in of this file with a real signal handler, and
 * obs_source_release() a recording stand-in, for both sides: each signal,
 * callback, release and log line is recorded in order with the lock state
 * at that moment. The real calls run through the real core in
 * test_source_filter_remove. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_filter_remove oracle_obs_source_filter_remove
#define obs_source_filter_remove_refless oracle_obs_source_filter_remove_refless
#include "libobs/build/libobs.pch.h"

#define SFR_FILTERS 7
#define SFR_EVENTS 16
/* One event: kind, three values, and whether filter_mutex was held. */
#define SFR_EVENT_WIDTH 5

/* Source ids. */
enum {
	SFR_NULL = -1,
	SFR_OTHER = -2,
	SFR_PARENT = -3,
	SFR_UNTOUCHED = -4,
	SFR_OUTSIDER = SFR_FILTERS,
};

/* Event kinds. */
enum {
	SFR_EV_GLOBAL_SIGNAL = 1,
	SFR_EV_SOURCE_SIGNAL,
	SFR_EV_CALLBACK,
	SFR_EV_RELEASE,
	SFR_EV_LOG,
};

struct sfr_fixture {
	struct obs_source parent;
	struct obs_source filters[SFR_FILTERS + 1];
	obs_source_t *list[SFR_FILTERS];
	struct obs_source parent_before;
	struct obs_source filters_before[SFR_FILTERS + 1];
	char markers[SFR_FILTERS + 1];
	long events[SFR_EVENTS][SFR_EVENT_WIDTH];
	size_t event_count;
	bool overflow;
	char message[128];
};

/* Per thread: the Rust test harness may run cases on several threads. */
static _Thread_local struct sfr_fixture *sfr_current;
static _Thread_local struct obs_core sfr_core;

static long sfr_id(const void *source)
{
	struct sfr_fixture *f = sfr_current;
	if (!source)
		return SFR_NULL;
	if (source == &f->parent)
		return SFR_PARENT;
	for (long i = 0; i <= SFR_FILTERS; i++) {
		if (source == &f->filters[i])
			return i;
	}
	return SFR_OTHER;
}

static bool sfr_locked(void)
{
	int busy = pthread_mutex_trylock(&sfr_current->parent.filter_mutex);
	if (busy == 0)
		pthread_mutex_unlock(&sfr_current->parent.filter_mutex);
	return busy != 0;
}

static void sfr_event(long kind, long a, long b, long c)
{
	struct sfr_fixture *f = sfr_current;
	if (f->event_count >= SFR_EVENTS) {
		f->overflow = true;
		return;
	}
	long *e = f->events[f->event_count++];
	e[0] = kind;
	e[1] = a;
	e[2] = b;
	e[3] = c;
	e[4] = sfr_locked();
}

static void sfr_release(obs_source_t *source)
{
	sfr_event(SFR_EV_RELEASE, sfr_id(source), 0, 0);
}

/* The calldata's source and filter, and the list's length then. */
static void sfr_signalled(long kind, calldata_t *data)
{
	sfr_event(kind, sfr_id(calldata_ptr(data, "source")), sfr_id(calldata_ptr(data, "filter")),
		  (long)sfr_current->parent.filters.num);
}

static void sfr_global_signal(void *param, calldata_t *data)
{
	(void)param;
	sfr_signalled(SFR_EV_GLOBAL_SIGNAL, data);
}

static void sfr_source_signal(void *param, calldata_t *data)
{
	(void)param;
	sfr_signalled(SFR_EV_SOURCE_SIGNAL, data);
}

/* The filter (by its data), the parent passed, and the filter's own parent
 * field then. */
static void sfr_filter_remove(void *data, obs_source_t *parent)
{
	struct sfr_fixture *f = sfr_current;
	long i = (long)((char *)data - f->markers);
	long field = i >= 0 && i <= SFR_FILTERS ? sfr_id(f->filters[i].filter_parent) : SFR_OTHER;
	sfr_event(SFR_EV_CALLBACK, i, sfr_id(parent), field);
}

#define obs sfr_obs
static _Thread_local struct obs_core *sfr_obs;
#define obs_source_release sfr_release
#include "source-filter-remove.inc"
#undef obs_source_filter_remove
#undef obs_source_filter_remove_refless
#include "source_filter_remove_fields.inc"
#undef obs_source_release
#undef obs

static char sfr_parent_name[] = "parent";
static char sfr_names[SFR_FILTERS + 1][9] = {"a", "bb", "", "dddd", "e", "ffffff", "g", "outsider"};
static const char *const sfr_ids[SFR_FILTERS + 1] = {"f0", "f1", "f2", "f3", "f4", "f5", "f6", "f7"};

static void sfr_free(struct sfr_fixture *f, bool *ok)
{
	signal_handler_destroy(f->parent.context.signals);
	signal_handler_destroy(sfr_core.signals);
	sfr_core.signals = NULL;
	if (pthread_mutex_destroy(&f->parent.filter_mutex) != 0)
		*ok = false;
	free(f);
}

static struct sfr_fixture *sfr_new(uint8_t noise, size_t count, unsigned callbacks)
{
	struct sfr_fixture *f = malloc(sizeof(*f));
	if (!f)
		return NULL;
	memset(f, noise, sizeof(*f));
	if (pthread_mutex_init(&f->parent.filter_mutex, NULL) != 0) {
		free(f);
		return NULL;
	}
	f->parent.context.signals = signal_handler_create();
	sfr_core.signals = signal_handler_create();
	if (!f->parent.context.signals || !sfr_core.signals ||
	    !signal_handler_add(f->parent.context.signals, "void filter_remove(ptr source, ptr filter)") ||
	    !signal_handler_add(sfr_core.signals, "void source_filter_remove(ptr source, ptr filter)")) {
		bool ignored = true;
		sfr_free(f, &ignored);
		return NULL;
	}
	signal_handler_connect(f->parent.context.signals, "filter_remove", sfr_source_signal, NULL);
	signal_handler_connect(sfr_core.signals, "source_filter_remove", sfr_global_signal, NULL);
	f->parent.context.name = sfr_parent_name;
	for (size_t i = 0; i <= SFR_FILTERS; i++) {
		struct obs_source *filter = &f->filters[i];
		filter->context.name = sfr_names[i];
		filter->info.id = sfr_ids[i];
		filter->context.data = &f->markers[i];
		filter->info.filter_remove = (callbacks & (1u << i)) ? sfr_filter_remove : NULL;
		if (i < count) {
			filter->filter_parent = &f->parent;
			filter->filter_target = i + 1 < count ? &f->filters[i + 1] : &f->parent;
		}
		if (i < SFR_FILTERS)
			f->list[i] = filter;
	}
	f->parent.filters.array = f->list;
	f->parent.filters.num = count;
	f->parent.filters.capacity = SFR_FILTERS;
	f->event_count = 0;
	f->overflow = false;
	f->message[0] = '\0';
	memcpy(&f->parent_before, &f->parent, sizeof(f->parent));
	memcpy(f->filters_before, f->filters, sizeof(f->filters));
	return f;
}

/* Whether before and after differ only inside the given field ranges. */
static bool sfr_equal_except(const void *before, const void *after, size_t size, size_t at1, size_t len1, size_t at2,
			     size_t len2)
{
	const unsigned char *b = before;
	const unsigned char *a = after;
	for (size_t i = 0; i < size; i++) {
		if ((i >= at1 && i < at1 + len1) || (i >= at2 && i < at2 + len2))
			continue;
		if (a[i] != b[i])
			return false;
	}
	return true;
}

/* What a filter_target or filter_parent holds after the call. */
static long sfr_link(const void *before, const void *after)
{
	const obs_source_t *value;
	if (memcmp(before, after, sizeof(value)) == 0)
		return SFR_UNTOUCHED;
	memcpy(&value, after, sizeof(value));
	return sfr_id(value);
}

struct sfr_log {
	unsigned count;
};
#if defined(__GNUC__) || defined(__clang__)
static void sfr_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void sfr_capture(int level, const char *format, va_list args, void *param)
{
	struct sfr_log *captured = param;
	captured->count++;
	vsnprintf(sfr_current->message, sizeof(sfr_current->message), format, args);
	sfr_event(SFR_EV_LOG, level, (long)captured->count, 0);
}

static void sfr_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		len = size - 1;
	memcpy(dst, src, len);
	dst[len] = '\0';
}

typedef void (*sfr_remove)(obs_source_t *, obs_source_t *);
typedef bool (*sfr_refless)(obs_source_t *, obs_source_t *);
typedef void (*sfr_get_log)(log_handler_t *, void **);
typedef void (*sfr_set_log)(log_handler_t, void *);

/* The out array, by index. */
enum {
	SFR_OUT_RETURNED = 0,
	SFR_OUT_COUNT_AFTER,
	SFR_OUT_ORDER,
	SFR_OUT_TARGETS = SFR_OUT_ORDER + SFR_FILTERS,
	SFR_OUT_PARENTS = SFR_OUT_TARGETS + SFR_FILTERS + 1,
	SFR_OUT_UNCHANGED = SFR_OUT_PARENTS + SFR_FILTERS + 1,
	SFR_OUT_UNLOCKED,
	SFR_OUT_LOGS,
	SFR_OUT_EVENTS,
	SFR_OUT_OVERFLOW,
	SFR_OUT_HEAD,
	SFR_OUT_COUNT = SFR_OUT_HEAD + SFR_EVENTS * SFR_EVENT_WIDTH,
};

/* Runs one call on a fresh fixture of count filters (filter i has a
 * filter_remove callback when bit i of callbacks is set): with refless
 * false obs_source_filter_remove() through `remove` (the Rust export or a
 * mutant; NULL for the original) on the parent (NULL when present is
 * false), else obs_source_filter_remove_refless() through `refless` (NULL
 * for the original) on the parent. `which` picks the filter: an index, the
 * outsider (SFR_FILTERS, on no list) or, anything larger, NULL. Writes
 * SFR_OUT_COUNT values to out and the last log line to message; false when
 * the fixture could not be built. */
bool oracle_sfr_run(sfr_remove remove, sfr_refless refless_fn, sfr_get_log get_log, sfr_set_log set_log, bool refless,
		    bool present, uint8_t noise, size_t count, unsigned callbacks, size_t which, long *out,
		    char *message, size_t message_size);
bool oracle_sfr_run(sfr_remove remove, sfr_refless refless_fn, sfr_get_log get_log, sfr_set_log set_log, bool refless,
		    bool present, uint8_t noise, size_t count, unsigned callbacks, size_t which, long *out,
		    char *message, size_t message_size)
{
	if (count > SFR_FILTERS || message_size == 0)
		return false;
	struct sfr_fixture *f = sfr_new(noise, count, callbacks);
	if (!f)
		return false;
	obs_source_t *filter = which <= SFR_FILTERS ? &f->filters[which] : NULL;
	obs_source_t *source = present ? &f->parent : NULL;
	struct sfr_log captured = {0};
	log_handler_t previous;
	void *param;
	bool returned = false;

	sfr_current = f;
	sfr_obs = &sfr_core;
	get_log(&previous, &param);
	set_log(sfr_capture, &captured);
	if (refless) {
		if (refless_fn)
			returned = refless_fn(source, filter);
		else
			returned = oracle_obs_source_filter_remove_refless(source, filter);
	} else {
		if (remove)
			remove(source, filter);
		else
			oracle_obs_source_filter_remove(source, filter);
	}
	set_log(previous, param);

	memset(out, 0, sizeof(long) * SFR_OUT_COUNT);
	out[SFR_OUT_RETURNED] = returned;
	out[SFR_OUT_COUNT_AFTER] = (long)f->parent.filters.num;
	for (size_t i = 0; i < SFR_FILTERS; i++)
		out[SFR_OUT_ORDER + i] = sfr_id(f->list[i]);
	for (size_t i = 0; i <= SFR_FILTERS; i++) {
		out[SFR_OUT_TARGETS + i] = sfr_link(&f->filters_before[i].filter_target, &f->filters[i].filter_target);
		out[SFR_OUT_PARENTS + i] = sfr_link(&f->filters_before[i].filter_parent, &f->filters[i].filter_parent);
	}

	bool unchanged = sfr_equal_except(&f->parent_before, &f->parent, sizeof(f->parent),
					  offsetof(struct obs_source, filter_mutex), sizeof(f->parent.filter_mutex),
					  offsetof(struct obs_source, filters.num), sizeof(f->parent.filters.num));
	for (size_t i = 0; i <= SFR_FILTERS; i++)
		unchanged = unchanged && sfr_equal_except(&f->filters_before[i], &f->filters[i], sizeof(f->filters[i]),
							  offsetof(struct obs_source, filter_target),
							  sizeof(f->filters[i].filter_target),
							  offsetof(struct obs_source, filter_parent),
							  sizeof(f->filters[i].filter_parent));
	out[SFR_OUT_UNCHANGED] = unchanged;
	out[SFR_OUT_UNLOCKED] = !sfr_locked();
	out[SFR_OUT_LOGS] = (long)captured.count;
	out[SFR_OUT_EVENTS] = (long)f->event_count;
	out[SFR_OUT_OVERFLOW] = f->overflow;
	for (size_t i = 0; i < f->event_count; i++)
		memcpy(&out[SFR_OUT_HEAD + i * SFR_EVENT_WIDTH], f->events[i], sizeof(f->events[i]));
	sfr_copy(message, message_size, f->message);

	bool ok = true;
	sfr_current = NULL;
	sfr_obs = NULL;
	sfr_free(f, &ok);
	return ok;
}

/* The sizes the Rust side mirrors: 0 the out array, 1 the most filters,
 * 2 the event width. */
size_t oracle_sfr_layout(size_t which);
size_t oracle_sfr_layout(size_t which)
{
	return which == 0 ? SFR_OUT_COUNT : which == 1 ? SFR_FILTERS : which == 2 ? SFR_EVENT_WIDTH : 0;
}
