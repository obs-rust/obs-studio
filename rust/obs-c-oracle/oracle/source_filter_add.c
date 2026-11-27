/* Tier 3: the unchanged extracted obs_source_filter_add()
 * (libobs/obs-source-filter-add.c.h) over the real private layout, and the
 * production field adapters the Rust shim calls
 * (obs-source-filter-add-fields.c.h, defined once, here).
 *
 * The original calls the filter chain rules by their libobs names, so it
 * runs on the rules hybrid libobs ships (obs-source's source_filter_chain),
 * and filter_compatible() is its libobs body over those rules. libobs's core
 * global is a stand-in of this file with a real signal handler, and
 * obs_source_get_ref() a recording stand-in, for both sides: each reference,
 * signal, callback and log line is recorded in order with the lock state at
 * that moment. The real calls run through the real core in
 * test_source_filter_add. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_filter_add oracle_obs_source_filter_add
#include "libobs/build/libobs.pch.h"

/* Filters on the fixture: up to SFA_SLOTS - 1 listed, so one more fits the
 * list without growing it. */
#define SFA_SLOTS 8
#define SFA_EVENTS 16
/* One event: kind, three values, and whether filter_mutex was held. */
#define SFA_EVENT_WIDTH 5

/* Source ids. */
enum {
	SFA_NULL = -1,
	SFA_OTHER = -2,
	SFA_PARENT = -3,
	SFA_UNTOUCHED = -4,
	SFA_ELSEWHERE = -5,
};

/* Event kinds. */
enum {
	SFA_EV_GLOBAL_SIGNAL = 1,
	SFA_EV_SOURCE_SIGNAL,
	SFA_EV_CALLBACK,
	SFA_EV_REF,
	SFA_EV_LOG,
};

struct sfa_fixture {
	struct obs_source parent;
	struct obs_source elsewhere;
	struct obs_source filters[SFA_SLOTS];
	obs_source_t *list[SFA_SLOTS];
	struct obs_source parent_before;
	struct obs_source filters_before[SFA_SLOTS];
	char markers[SFA_SLOTS];
	bool dying;
	long events[SFA_EVENTS][SFA_EVENT_WIDTH];
	size_t event_count;
	bool overflow;
	char message[128];
};

/* Per thread: the Rust test harness may run cases on several threads. */
static _Thread_local struct sfa_fixture *sfa_current;
static _Thread_local struct obs_core sfa_core;

static long sfa_id(const void *source)
{
	struct sfa_fixture *f = sfa_current;
	if (!source)
		return SFA_NULL;
	if (source == &f->parent)
		return SFA_PARENT;
	if (source == &f->elsewhere)
		return SFA_ELSEWHERE;
	for (long i = 0; i < SFA_SLOTS; i++) {
		if (source == &f->filters[i])
			return i;
	}
	return SFA_OTHER;
}

static bool sfa_locked(void)
{
	int busy = pthread_mutex_trylock(&sfa_current->parent.filter_mutex);
	if (busy == 0)
		pthread_mutex_unlock(&sfa_current->parent.filter_mutex);
	return busy != 0;
}

static void sfa_event(long kind, long a, long b, long c)
{
	struct sfa_fixture *f = sfa_current;
	if (f->event_count >= SFA_EVENTS) {
		f->overflow = true;
		return;
	}
	long *e = f->events[f->event_count++];
	e[0] = kind;
	e[1] = a;
	e[2] = b;
	e[3] = c;
	e[4] = sfa_locked();
}

/* obs_source_get_ref(): NULL for the fixture's dying filter. */
static obs_source_t *sfa_get_ref(obs_source_t *source)
{
	struct sfa_fixture *f = sfa_current;
	sfa_event(SFA_EV_REF, sfa_id(source), (long)f->dying, 0);
	return f->dying ? NULL : source;
}

/* The calldata's source and filter, the list's length and its first
 * entry then. */
static void sfa_signalled(long kind, calldata_t *data)
{
	struct sfa_fixture *f = sfa_current;
	long first = f->parent.filters.num ? sfa_id(f->parent.filters.array[0]) : SFA_NULL;
	sfa_event(kind, sfa_id(calldata_ptr(data, "source")), sfa_id(calldata_ptr(data, "filter")),
		  (long)f->parent.filters.num * 16 + first);
}

static void sfa_global_signal(void *param, calldata_t *data)
{
	(void)param;
	sfa_signalled(SFA_EV_GLOBAL_SIGNAL, data);
}

static void sfa_source_signal(void *param, calldata_t *data)
{
	(void)param;
	sfa_signalled(SFA_EV_SOURCE_SIGNAL, data);
}

/* The filter (by its data), the parent passed, and the filter's own parent
 * and target fields then. */
static void sfa_filter_add(void *data, obs_source_t *parent)
{
	struct sfa_fixture *f = sfa_current;
	long i = (long)((char *)data - f->markers);
	long field = i >= 0 && i < SFA_SLOTS
			     ? sfa_id(f->filters[i].filter_parent) * 64 + sfa_id(f->filters[i].filter_target)
			     : SFA_OTHER;
	sfa_event(SFA_EV_CALLBACK, i, sfa_id(parent), field);
}

/* filter_compatible() of obs-source.c.h, unchanged. */
static bool sfa_compatible(obs_source_t *source, obs_source_t *filter)
{
	return obs_source_filter_chain_compatible(source->info.output_flags, filter->info.output_flags);
}

#define obs sfa_obs
static _Thread_local struct obs_core *sfa_obs;
#define obs_source_get_ref sfa_get_ref
#define filter_compatible sfa_compatible
#include "source-filter-add.inc"
#undef obs_source_filter_add
#include "source_filter_add_fields.inc"
#undef filter_compatible
#undef obs_source_get_ref
#undef obs

static char sfa_parent_name[] = "parent";
static char sfa_names[SFA_SLOTS][9] = {"a", "bb", "", "dddd", "e", "ffffff", "g", "newcomer"};
static const char *const sfa_ids[SFA_SLOTS] = {"f0", "f1", "f2", "f3", "f4", "f5", "f6", "f7"};

static void sfa_free(struct sfa_fixture *f, bool *ok)
{
	signal_handler_destroy(f->parent.context.signals);
	signal_handler_destroy(sfa_core.signals);
	sfa_core.signals = NULL;
	if (pthread_mutex_destroy(&f->parent.filter_mutex) != 0)
		*ok = false;
	free(f);
}

/* A fresh fixture: filters 0 to count - 1 listed (filter i targets the next
 * one, the last the parent); a filter not listed has the other source as
 * parent when its bit of parented is set; filter i has a filter_add
 * callback when its bit of callbacks is set. */
static struct sfa_fixture *sfa_new(uint8_t noise, size_t count, unsigned parented, unsigned callbacks,
				   uint32_t source_flags, uint32_t filter_flags, bool owns_info_id, bool dying)
{
	struct sfa_fixture *f = malloc(sizeof(*f));
	if (!f)
		return NULL;
	memset(f, noise, sizeof(*f));
	if (pthread_mutex_init(&f->parent.filter_mutex, NULL) != 0) {
		free(f);
		return NULL;
	}
	f->parent.context.signals = signal_handler_create();
	sfa_core.signals = signal_handler_create();
	if (!f->parent.context.signals || !sfa_core.signals ||
	    !signal_handler_add(f->parent.context.signals, "void filter_add(ptr source, ptr filter)") ||
	    !signal_handler_add(sfa_core.signals, "void source_filter_add(ptr source, ptr filter)")) {
		bool ignored = true;
		sfa_free(f, &ignored);
		return NULL;
	}
	signal_handler_connect(f->parent.context.signals, "filter_add", sfa_source_signal, NULL);
	signal_handler_connect(sfa_core.signals, "source_filter_add", sfa_global_signal, NULL);
	f->parent.context.name = sfa_parent_name;
	f->parent.info.output_flags = source_flags;
	f->parent.owns_info_id = owns_info_id;
	for (size_t i = 0; i < SFA_SLOTS; i++) {
		struct obs_source *filter = &f->filters[i];
		filter->context.name = sfa_names[i];
		filter->info.id = sfa_ids[i];
		filter->info.output_flags = filter_flags;
		filter->context.data = &f->markers[i];
		filter->info.filter_add = (callbacks & (1u << i)) ? sfa_filter_add : NULL;
		if (i < count) {
			filter->filter_parent = &f->parent;
			filter->filter_target = i + 1 < count ? &f->filters[i + 1] : &f->parent;
		} else if (parented & (1u << i)) {
			filter->filter_parent = &f->elsewhere;
		} else {
			filter->filter_parent = NULL;
		}
		f->list[i] = filter;
	}
	f->parent.filters.array = f->list;
	f->parent.filters.num = count;
	f->parent.filters.capacity = SFA_SLOTS;
	f->dying = dying;
	f->event_count = 0;
	f->overflow = false;
	f->message[0] = '\0';
	memcpy(&f->parent_before, &f->parent, sizeof(f->parent));
	memcpy(f->filters_before, f->filters, sizeof(f->filters));
	return f;
}

/* Whether before and after differ only inside the given field ranges. */
static bool sfa_equal_except(const void *before, const void *after, size_t size, size_t at1, size_t len1, size_t at2,
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
static long sfa_link(const void *before, const void *after)
{
	const obs_source_t *value;
	if (memcmp(before, after, sizeof(value)) == 0)
		return SFA_UNTOUCHED;
	memcpy(&value, after, sizeof(value));
	return sfa_id(value);
}

struct sfa_log {
	unsigned count;
};
#if defined(__GNUC__) || defined(__clang__)
static void sfa_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void sfa_capture(int level, const char *format, va_list args, void *param)
{
	struct sfa_log *captured = param;
	captured->count++;
	vsnprintf(sfa_current->message, sizeof(sfa_current->message), format, args);
	sfa_event(SFA_EV_LOG, level, (long)captured->count, 0);
}

static void sfa_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		len = size - 1;
	memcpy(dst, src, len);
	dst[len] = '\0';
}

typedef void (*sfa_add)(obs_source_t *, obs_source_t *);
typedef void (*sfa_get_log)(log_handler_t *, void **);
typedef void (*sfa_set_log)(log_handler_t, void *);

/* The out array, by index. */
enum {
	SFA_OUT_COUNT_AFTER = 0,
	SFA_OUT_ORDER,
	SFA_OUT_TARGETS = SFA_OUT_ORDER + SFA_SLOTS,
	SFA_OUT_PARENTS = SFA_OUT_TARGETS + SFA_SLOTS,
	SFA_OUT_UNCHANGED = SFA_OUT_PARENTS + SFA_SLOTS,
	SFA_OUT_UNLOCKED,
	SFA_OUT_LOGS,
	SFA_OUT_EVENTS,
	SFA_OUT_OVERFLOW,
	SFA_OUT_HEAD,
	SFA_OUT_COUNT = SFA_OUT_HEAD + SFA_EVENTS * SFA_EVENT_WIDTH,
};

/* Runs one obs_source_filter_add() through `add` (the Rust export or a
 * mutant; NULL for the original) on a fresh fixture (see sfa_new()), on the
 * parent (NULL when present is false). `which` picks the filter: an index
 * below SFA_SLOTS (listed when below count) or, anything larger, NULL; the
 * reference fails when dying is set. Writes SFA_OUT_COUNT values to out and
 * the last log line to message; false when the fixture could not be
 * built. */
bool oracle_sfa_run(sfa_add add, sfa_get_log get_log, sfa_set_log set_log, bool present, uint8_t noise, size_t count,
		    unsigned parented, unsigned callbacks, uint32_t source_flags, uint32_t filter_flags,
		    bool owns_info_id, bool dying, size_t which, long *out, char *message, size_t message_size);
bool oracle_sfa_run(sfa_add add, sfa_get_log get_log, sfa_set_log set_log, bool present, uint8_t noise, size_t count,
		    unsigned parented, unsigned callbacks, uint32_t source_flags, uint32_t filter_flags,
		    bool owns_info_id, bool dying, size_t which, long *out, char *message, size_t message_size)
{
	if (count >= SFA_SLOTS || message_size == 0)
		return false;
	struct sfa_fixture *f =
		sfa_new(noise, count, parented, callbacks, source_flags, filter_flags, owns_info_id, dying);
	if (!f)
		return false;
	obs_source_t *filter = which < SFA_SLOTS ? &f->filters[which] : NULL;
	obs_source_t *source = present ? &f->parent : NULL;
	struct sfa_log captured = {0};
	log_handler_t previous;
	void *param;

	sfa_current = f;
	sfa_obs = &sfa_core;
	get_log(&previous, &param);
	set_log(sfa_capture, &captured);
	if (add)
		add(source, filter);
	else
		oracle_obs_source_filter_add(source, filter);
	set_log(previous, param);

	memset(out, 0, sizeof(long) * SFA_OUT_COUNT);
	out[SFA_OUT_COUNT_AFTER] = (long)f->parent.filters.num;
	for (size_t i = 0; i < SFA_SLOTS; i++) {
		out[SFA_OUT_ORDER + i] = i < f->parent.filters.num ? sfa_id(f->parent.filters.array[i]) : SFA_UNTOUCHED;
		out[SFA_OUT_TARGETS + i] = sfa_link(&f->filters_before[i].filter_target, &f->filters[i].filter_target);
		out[SFA_OUT_PARENTS + i] = sfa_link(&f->filters_before[i].filter_parent, &f->filters[i].filter_parent);
	}

	/* The list may only change its length and, when it grew, its entries
	 * (compared above); the array and capacity stay. */
	bool unchanged = sfa_equal_except(&f->parent_before, &f->parent, sizeof(f->parent),
					  offsetof(struct obs_source, filter_mutex), sizeof(f->parent.filter_mutex),
					  offsetof(struct obs_source, filters.num), sizeof(f->parent.filters.num));
	for (size_t i = 0; i < SFA_SLOTS; i++)
		unchanged = unchanged && sfa_equal_except(&f->filters_before[i], &f->filters[i], sizeof(f->filters[i]),
							  offsetof(struct obs_source, filter_target),
							  sizeof(f->filters[i].filter_target),
							  offsetof(struct obs_source, filter_parent),
							  sizeof(f->filters[i].filter_parent));
	out[SFA_OUT_UNCHANGED] = unchanged;
	out[SFA_OUT_UNLOCKED] = !sfa_locked();
	out[SFA_OUT_LOGS] = (long)captured.count;
	out[SFA_OUT_EVENTS] = (long)f->event_count;
	out[SFA_OUT_OVERFLOW] = f->overflow;
	for (size_t i = 0; i < f->event_count; i++)
		memcpy(&out[SFA_OUT_HEAD + i * SFA_EVENT_WIDTH], f->events[i], sizeof(f->events[i]));
	sfa_copy(message, message_size, f->message);

	bool ok = true;
	sfa_current = NULL;
	sfa_obs = NULL;
	/* A lock left held (the C's answer for a filter that can't be
	 * referenced, never given by the shipped rule) is released before the
	 * mutex is destroyed. */
	if (!out[SFA_OUT_UNLOCKED])
		pthread_mutex_unlock(&f->parent.filter_mutex);
	sfa_free(f, &ok);
	return ok;
}

/* The sizes the Rust side mirrors: 0 the out array, 1 the filter slots,
 * 2 the event width. */
size_t oracle_sfa_layout(size_t which);
size_t oracle_sfa_layout(size_t which)
{
	return which == 0 ? SFA_OUT_COUNT : which == 1 ? SFA_SLOTS : which == 2 ? SFA_EVENT_WIDTH : 0;
}
