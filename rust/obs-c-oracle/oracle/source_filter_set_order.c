/* Tier 3: the unchanged extracted obs_source_filter_set_order()
 * (libobs/obs-source-filter-set-order.c.h) over the real private layout,
 * and the production field adapters the Rust shim calls
 * (obs-source-filter-set-order-fields.c.h, defined once, here).
 *
 * The original calls the filter chain rules by their libobs names, so it
 * runs on the rules hybrid libobs ships (obs-source's
 * source_filter_chain; their own parity is source_filter_chain_parity). Its
 * retarget, reorder_filter_targets(), stays in obs-source.c.h, which the
 * oracle does not compile: sfo_reorder_filter_targets() below is that
 * function's body. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_filter_set_order oracle_obs_source_filter_set_order
#define reorder_filter_targets sfo_reorder_filter_targets
#include "libobs/build/libobs.pch.h"

/* obs_source_dosignal() (obs-internal.h) for both sides, without the core
 * the oracle has none of: a global signal name is only counted; the
 * source's own signal goes through its handler with the same calldata. */
static size_t sfo_global_signals;
static void sfo_dosignal(struct obs_source *source, const char *signal_obs, const char *signal_source)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	if (signal_obs)
		sfo_global_signals++;
	if (signal_source)
		signal_handler_signal(source->context.signals, signal_source, &data);
}

#define obs_source_dosignal sfo_dosignal
#include "source-filter-set-order.inc"
#undef obs_source_filter_set_order
#undef reorder_filter_targets
#include "source_filter_set_order_fields.inc"
#undef obs_source_dosignal

/* obs-source.c.h's reorder_filter_targets(), unchanged. */
void sfo_reorder_filter_targets(obs_source_t *source)
{
	/* reorder filter targets, not the nicest way of dealing with things */
	for (size_t i = 0; i < source->filters.num; i++) {
		obs_source_t *next_filter = obs_source_filter_chain_target_of(i, source->filters.num) ==
							    OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE
						    ? source
						    : source->filters.array[i + 1];

		source->filters.array[i]->filter_target = next_filter;
	}
}

#define SFO_FILTERS 7

/* What a filter_target holds after a call. */
enum {
	SFO_TARGET_UNTOUCHED = -3,
	SFO_TARGET_SOURCE = -2,
	SFO_TARGET_OTHER = -1,
};

/* The out array of oracle_sfo_run(), by index. */
enum {
	SFO_OUT_ORDER = 0,
	SFO_OUT_TARGETS = SFO_OUT_ORDER + SFO_FILTERS,
	SFO_OUT_SIGNALS = SFO_OUT_TARGETS + SFO_FILTERS,
	SFO_OUT_SIGNAL_SOURCE,
	SFO_OUT_SIGNAL_UNLOCKED,
	SFO_OUT_SIGNAL_LIST,
	SFO_OUT_UNCHANGED,
	SFO_OUT_UNLOCKED,
	SFO_OUT_LOGS,
	SFO_OUT_LOG_LEVEL,
	SFO_OUT_GLOBAL_SIGNALS,
	SFO_OUT_COUNT,
};

struct sfo_fixture {
	struct obs_source parent;
	struct obs_source filters[SFO_FILTERS];
	struct obs_source outsider;
	obs_source_t *list[SFO_FILTERS];
	struct obs_source parent_before;
	struct obs_source filters_before[SFO_FILTERS];
	struct obs_source outsider_before;
	size_t signals;
	void *signal_source;
	bool signal_unlocked;
	obs_source_t *signal_list[SFO_FILTERS];
};

static void sfo_reorder_filters(void *param, calldata_t *data)
{
	struct sfo_fixture *f = param;
	f->signals++;
	f->signal_source = calldata_ptr(data, "source");
	int locked = pthread_mutex_trylock(&f->parent.filter_mutex);
	if (locked == 0)
		pthread_mutex_unlock(&f->parent.filter_mutex);
	f->signal_unlocked = locked == 0;
	memcpy(f->signal_list, f->list, sizeof(f->list));
}

static struct sfo_fixture *sfo_new(uint8_t noise, size_t count, unsigned async)
{
	struct sfo_fixture *f = malloc(sizeof(*f));
	if (!f)
		return NULL;
	memset(f, noise, sizeof(*f));
	if (pthread_mutex_init(&f->parent.filter_mutex, NULL) != 0) {
		free(f);
		return NULL;
	}
	f->parent.context.signals = signal_handler_create();
	if (!f->parent.context.signals ||
	    !signal_handler_add(f->parent.context.signals, "void reorder_filters(ptr source)")) {
		signal_handler_destroy(f->parent.context.signals);
		pthread_mutex_destroy(&f->parent.filter_mutex);
		free(f);
		return NULL;
	}
	signal_handler_connect(f->parent.context.signals, "reorder_filters", sfo_reorder_filters, f);
	for (size_t i = 0; i < SFO_FILTERS; i++) {
		uint32_t flags = OBS_SOURCE_VIDEO | ((uint32_t)noise << 8);
		if (async & (1u << i))
			flags |= OBS_SOURCE_ASYNC;
		f->filters[i].info.output_flags = flags;
		f->list[i] = &f->filters[i];
	}
	f->outsider.info.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_ASYNC;
	f->parent.filters.array = f->list;
	f->parent.filters.num = count;
	f->parent.filters.capacity = SFO_FILTERS;
	f->signals = 0;
	f->signal_source = NULL;
	f->signal_unlocked = false;
	memcpy(&f->parent_before, &f->parent, sizeof(f->parent));
	memcpy(f->filters_before, f->filters, sizeof(f->filters));
	memcpy(&f->outsider_before, &f->outsider, sizeof(f->outsider));
	return f;
}

static bool sfo_free(struct sfo_fixture *f)
{
	signal_handler_destroy(f->parent.context.signals);
	bool ok = pthread_mutex_destroy(&f->parent.filter_mutex) == 0;
	free(f);
	return ok;
}

static bool sfo_equal_except(const void *before, const void *after, size_t size, size_t at, size_t len)
{
	const unsigned char *b = before;
	const unsigned char *a = after;
	return memcmp(b, a, at) == 0 && memcmp(b + at + len, a + at + len, size - at - len) == 0;
}

static long sfo_index(struct sfo_fixture *f, const obs_source_t *filter)
{
	for (size_t i = 0; i < SFO_FILTERS; i++) {
		if (filter == &f->filters[i])
			return (long)i;
	}
	return -1;
}

static long sfo_target(struct sfo_fixture *f, size_t i)
{
	const size_t at = offsetof(struct obs_source, filter_target);
	if (memcmp((const unsigned char *)&f->filters_before[i] + at, (const unsigned char *)&f->filters[i] + at,
		   sizeof(f->filters[i].filter_target)) == 0)
		return SFO_TARGET_UNTOUCHED;
	if (f->filters[i].filter_target == &f->parent)
		return SFO_TARGET_SOURCE;
	long index = sfo_index(f, f->filters[i].filter_target);
	return index < 0 ? SFO_TARGET_OTHER : index;
}

struct sfo_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void sfo_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void sfo_capture(int level, const char *format, va_list args, void *param)
{
	struct sfo_log *captured = param;
	captured->count++;
	captured->level = level;
	vsnprintf(captured->message, sizeof(captured->message), format, args);
}

static void sfo_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		len = size - 1;
	memcpy(dst, src, len);
	dst[len] = '\0';
}

typedef void (*sfo_set_order)(obs_source_t *, obs_source_t *, int);
typedef void (*sfo_get_log)(log_handler_t *, void **);
typedef void (*sfo_set_log)(log_handler_t, void *);

/* Runs set_order (the Rust export or a mutant; NULL for the original) on a
 * fresh fixture of count filters, filter i async when bit i of async is
 * set: on the parent (NULL when present is false), with filter `which`
 * (an index, SFO_FILTERS for one not in the list, anything larger for
 * NULL) and movement. Writes what it observed to out[SFO_OUT_COUNT] and
 * the log line to message; false when the fixture could not be built. */
bool oracle_sfo_run(sfo_set_order set_order, sfo_get_log get_log, sfo_set_log set_log, bool present, uint8_t noise,
		    size_t count, unsigned async, size_t which, int movement, long *out, char *message,
		    size_t message_size);
bool oracle_sfo_run(sfo_set_order set_order, sfo_get_log get_log, sfo_set_log set_log, bool present, uint8_t noise,
		    size_t count, unsigned async, size_t which, int movement, long *out, char *message,
		    size_t message_size)
{
	if (count > SFO_FILTERS || message_size == 0)
		return false;
	struct sfo_fixture *f = sfo_new(noise, count, async);
	if (!f)
		return false;
	obs_source_t *filter = which < SFO_FILTERS ? &f->filters[which] : which == SFO_FILTERS ? &f->outsider : NULL;
	struct sfo_log captured = {.level = -1};
	log_handler_t previous;
	void *param;

	get_log(&previous, &param);
	sfo_global_signals = 0;
	set_log(sfo_capture, &captured);
	if (set_order)
		set_order(present ? &f->parent : NULL, filter, movement);
	else
		oracle_obs_source_filter_set_order(present ? &f->parent : NULL, filter,
						   (enum obs_order_movement)movement);
	set_log(previous, param);

	for (size_t i = 0; i < SFO_FILTERS; i++) {
		out[SFO_OUT_ORDER + i] = sfo_index(f, f->list[i]);
		out[SFO_OUT_TARGETS + i] = sfo_target(f, i);
	}
	out[SFO_OUT_SIGNALS] = (long)f->signals;
	out[SFO_OUT_SIGNAL_SOURCE] = f->signals == 0 || f->signal_source == &f->parent;
	out[SFO_OUT_SIGNAL_UNLOCKED] = f->signals == 0 || f->signal_unlocked;
	out[SFO_OUT_SIGNAL_LIST] = f->signals == 0 || memcmp(f->signal_list, f->list, sizeof(f->list)) == 0;

	bool unchanged = sfo_equal_except(&f->parent_before, &f->parent, sizeof(f->parent),
					  offsetof(struct obs_source, filter_mutex), sizeof(f->parent.filter_mutex)) &&
			 memcmp(&f->outsider_before, &f->outsider, sizeof(f->outsider)) == 0;
	for (size_t i = 0; i < SFO_FILTERS; i++)
		unchanged = unchanged && sfo_equal_except(&f->filters_before[i], &f->filters[i], sizeof(f->filters[i]),
							  offsetof(struct obs_source, filter_target),
							  sizeof(f->filters[i].filter_target));
	out[SFO_OUT_UNCHANGED] = unchanged;

	int locked = pthread_mutex_trylock(&f->parent.filter_mutex);
	if (locked == 0)
		pthread_mutex_unlock(&f->parent.filter_mutex);
	out[SFO_OUT_UNLOCKED] = locked == 0;
	out[SFO_OUT_LOGS] = (long)captured.count;
	out[SFO_OUT_LOG_LEVEL] = captured.level;
	out[SFO_OUT_GLOBAL_SIGNALS] = (long)sfo_global_signals;
	sfo_copy(message, message_size, captured.message);
	return sfo_free(f);
}

/* The sizes the Rust side mirrors: 0 the out array, 1 the most filters. */
size_t oracle_sfo_layout(size_t which);
size_t oracle_sfo_layout(size_t which)
{
	return which == 0 ? SFO_OUT_COUNT : which == 1 ? SFO_FILTERS : 0;
}
