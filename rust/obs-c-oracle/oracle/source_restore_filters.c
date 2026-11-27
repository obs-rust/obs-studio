/* Tier 3: the unchanged extracted obs_source_restore_filters()
 * (libobs/obs-source-restore-filters.c.h) over the real private layout, and
 * the production field adapters the Rust shim calls
 * (obs-source-restore-filters-fields.c.h, defined once, here).
 *
 * The original calls the filter chain rules by their libobs names, so it
 * runs on the rules hybrid libobs ships (obs-source's source_filter_chain).
 * For both sides, the array and its items are stand-ins of this file, and so
 * are obs_source_get_ref(), obs_source_release(), obs_load_source() and
 * obs_load_private_source(): each item read, name read, item release,
 * reference, load and release is recorded in order with the lock state and
 * a digest of the source's list and every filter's target and parent at
 * that moment. The real calls run through the real core in
 * test_source_restore_filters. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_restore_filters oracle_obs_source_restore_filters
#include "libobs/build/libobs.pch.h"

/* Current filters on the fixture, and array items (one loaded filter
 * each). */
#define SRF_SLOTS 6
#define SRF_ITEMS 6
#define SRF_LIST (SRF_SLOTS + SRF_ITEMS)
#define SRF_EVENTS 96
/* One event: kind, two values, the digest, and whether filter_mutex was
 * held. */
#define SRF_EVENT_WIDTH 5

/* Source ids: current filter i is i, the filter loaded for item k is
 * SRF_LOADED + k. */
enum {
	SRF_NULL = -1,
	SRF_OTHER = -2,
	SRF_PARENT = -3,
	SRF_UNTOUCHED = -4,
	SRF_LOADED = 100,
};

/* Event kinds. */
enum {
	SRF_EV_ITEM_COUNT = 1,
	SRF_EV_ITEM,
	SRF_EV_ITEM_NAME,
	SRF_EV_ITEM_RELEASE,
	SRF_EV_REF,
	SRF_EV_LOAD,
	SRF_EV_RELEASE,
	SRF_EV_LOG,
};

struct srf_item {
	const char *name;
};

struct srf_fixture {
	struct obs_source parent;
	struct obs_source filters[SRF_SLOTS];
	struct obs_source loaded[SRF_ITEMS];
	struct obs_source parent_before;
	struct obs_source filters_before[SRF_SLOTS];
	struct obs_source loaded_before[SRF_ITEMS];
	struct srf_item items[SRF_ITEMS];
	size_t item_count;
	char array_marker;
	long dying;
	long held;
	int64_t events[SRF_EVENTS][SRF_EVENT_WIDTH];
	size_t event_count;
	bool overflow;
	char message[128];
};

/* Per thread: the Rust test harness may run cases on several threads. */
static _Thread_local struct srf_fixture *srf_current;

static long srf_id(const void *source)
{
	struct srf_fixture *f = srf_current;
	if (!source)
		return SRF_NULL;
	if (source == &f->parent)
		return SRF_PARENT;
	for (long i = 0; i < SRF_SLOTS; i++) {
		if (source == &f->filters[i])
			return i;
	}
	for (long i = 0; i < SRF_ITEMS; i++) {
		if (source == &f->loaded[i])
			return SRF_LOADED + i;
	}
	return SRF_OTHER;
}

static bool srf_locked(void)
{
	int busy = pthread_mutex_trylock(&srf_current->parent.filter_mutex);
	if (busy == 0)
		pthread_mutex_unlock(&srf_current->parent.filter_mutex);
	return busy != 0;
}

/* FNV-1a over the list (read only when the lock is free: it is the
 * fixture's own thread) and every filter's target and parent. */
static int64_t srf_digest(void)
{
	struct srf_fixture *f = srf_current;
	uint64_t h = 1469598103934665603ull;
#define SRF_MIX(v)                                       \
	do {                                             \
		h ^= (uint64_t)(int64_t)(v);             \
		h *= 1099511628211ull;                   \
	} while (false)
	SRF_MIX(f->parent.filters.num);
	for (size_t i = 0; i < f->parent.filters.num && i < 64; i++)
		SRF_MIX(srf_id(f->parent.filters.array[i]));
	for (size_t i = 0; i < SRF_SLOTS; i++) {
		SRF_MIX(srf_id(f->filters[i].filter_target));
		SRF_MIX(srf_id(f->filters[i].filter_parent));
	}
	for (size_t i = 0; i < SRF_ITEMS; i++) {
		SRF_MIX(srf_id(f->loaded[i].filter_target));
		SRF_MIX(srf_id(f->loaded[i].filter_parent));
	}
#undef SRF_MIX
	return (int64_t)h;
}

static void srf_event(int64_t kind, int64_t a, int64_t b)
{
	struct srf_fixture *f = srf_current;
	if (f->event_count >= SRF_EVENTS) {
		f->overflow = true;
		return;
	}
	int64_t *e = f->events[f->event_count++];
	e[0] = kind;
	e[1] = a;
	e[2] = b;
	e[3] = srf_digest();
	e[4] = srf_locked();
}

static long srf_item_index(const obs_data_t *data)
{
	struct srf_fixture *f = srf_current;
	const struct srf_item *item = (const struct srf_item *)(const void *)data;
	if (item < f->items || item >= f->items + SRF_ITEMS)
		return SRF_OTHER;
	return (long)(item - f->items);
}

static size_t srf_array_count(obs_data_array_t *array)
{
	struct srf_fixture *f = srf_current;
	srf_event(SRF_EV_ITEM_COUNT, array == (obs_data_array_t *)(void *)&f->array_marker, 0);
	return f->item_count;
}

static obs_data_t *srf_array_item(obs_data_array_t *array, size_t idx)
{
	struct srf_fixture *f = srf_current;
	srf_event(SRF_EV_ITEM, (int64_t)idx, array == (obs_data_array_t *)(void *)&f->array_marker);
	if (idx >= f->item_count)
		return NULL;
	f->held++;
	return (obs_data_t *)(void *)&f->items[idx];
}

static const char *srf_get_string(obs_data_t *data, const char *name)
{
	long i = srf_item_index(data);
	srf_event(SRF_EV_ITEM_NAME, i, strcmp(name, "name") == 0);
	return i >= 0 ? srf_current->items[i].name : NULL;
}

static void srf_data_release(obs_data_t *data)
{
	srf_current->held--;
	srf_event(SRF_EV_ITEM_RELEASE, srf_item_index(data), 0);
}

/* obs_source_get_ref(): NULL for the fixture's dying filter. */
static obs_source_t *srf_get_ref(obs_source_t *source)
{
	struct srf_fixture *f = srf_current;
	long id = srf_id(source);
	srf_event(SRF_EV_REF, id, id == f->dying);
	return id == f->dying ? NULL : source;
}

static void srf_release(obs_source_t *source)
{
	srf_event(SRF_EV_RELEASE, srf_id(source), 0);
}

/* The filter loaded for an item; public tells the two loads apart. */
static obs_source_t *srf_load_as(obs_data_t *data, bool public)
{
	long i = srf_item_index(data);
	srf_event(SRF_EV_LOAD, i, public);
	return i >= 0 ? &srf_current->loaded[i] : NULL;
}

static obs_source_t *srf_load(obs_data_t *data)
{
	return srf_load_as(data, true);
}

static obs_source_t *srf_load_private(obs_data_t *data)
{
	return srf_load_as(data, false);
}

#define obs_data_array_count srf_array_count
#define obs_data_array_item srf_array_item
#define obs_data_get_string srf_get_string
#define obs_data_release srf_data_release
#define obs_source_get_ref srf_get_ref
#define obs_source_release srf_release
#define obs_load_source srf_load
#define obs_load_private_source srf_load_private
#include "source-restore-filters.inc"
#undef obs_source_restore_filters
#include "source_restore_filters_fields.inc"
#undef obs_load_private_source
#undef obs_load_source
#undef obs_source_release
#undef obs_source_get_ref
#undef obs_data_release
#undef obs_data_get_string
#undef obs_data_array_item
#undef obs_data_array_count

static char srf_parent_name[] = "parent";
/* Current filter names: an unnamed one and a repeated one. */
static char srf_names[SRF_SLOTS][3] = {"a", "bb", "", "e", "a", "g"};
static const bool srf_unnamed[SRF_SLOTS] = {false, false, true, false, false, false};
/* Item names, by selector: every current name, and two no filter has. */
static const char *const srf_item_names[] = {"a", "bb", "e", "g", "zz", "", "x"};
#define SRF_ITEM_NAMES (sizeof(srf_item_names) / sizeof(srf_item_names[0]))

static void srf_free(struct srf_fixture *f, bool *ok)
{
	da_free(f->parent.filters);
	if (pthread_mutex_destroy(&f->parent.filter_mutex) != 0)
		*ok = false;
	free(f);
}

/* A fresh fixture: filters 0 to count - 1 listed (filter i targets the next
 * one, the last the parent), item k named after selector (items >> 3k) & 7
 * for k below item_count; dying is the id that can't be referenced (or
 * SRF_NULL). */
static struct srf_fixture *srf_new(uint8_t noise, size_t count, size_t item_count, uint32_t items, long dying)
{
	struct srf_fixture *f = malloc(sizeof(*f));
	if (!f)
		return NULL;
	memset(f, noise, sizeof(*f));
	if (pthread_mutex_init(&f->parent.filter_mutex, NULL) != 0) {
		free(f);
		return NULL;
	}
	f->parent.context.name = srf_parent_name;
	f->parent.filters.array = NULL;
	f->parent.filters.num = 0;
	f->parent.filters.capacity = 0;
	for (size_t i = 0; i < SRF_SLOTS; i++) {
		struct obs_source *filter = &f->filters[i];
		filter->context.name = srf_unnamed[i] ? NULL : srf_names[i];
		if (i < count) {
			obs_source_t *entry = filter;
			da_push_back(f->parent.filters, &entry);
			filter->filter_parent = &f->parent;
			filter->filter_target = i + 1 < count ? &f->filters[i + 1] : &f->parent;
		} else {
			filter->filter_parent = NULL;
			filter->filter_target = NULL;
		}
	}
	for (size_t i = 0; i < SRF_ITEMS; i++) {
		f->loaded[i].filter_parent = NULL;
		f->loaded[i].filter_target = NULL;
		f->loaded[i].context.name = NULL;
		uint32_t selector = (items >> (3 * i)) & 7;
		f->items[i].name = srf_item_names[selector % SRF_ITEM_NAMES];
	}
	f->item_count = item_count;
	f->dying = dying;
	f->held = 0;
	f->event_count = 0;
	f->overflow = false;
	f->message[0] = '\0';
	memcpy(&f->parent_before, &f->parent, sizeof(f->parent));
	memcpy(f->filters_before, f->filters, sizeof(f->filters));
	memcpy(f->loaded_before, f->loaded, sizeof(f->loaded));
	return f;
}

/* Whether before and after differ only inside the given field ranges. */
static bool srf_equal_except(const void *before, const void *after, size_t size, size_t at1, size_t len1, size_t at2,
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
static long srf_link(const void *before, const void *after)
{
	const obs_source_t *value;
	if (memcmp(before, after, sizeof(value)) == 0)
		return SRF_UNTOUCHED;
	memcpy(&value, after, sizeof(value));
	return srf_id(value);
}

struct srf_log {
	unsigned count;
};
#if defined(__GNUC__) || defined(__clang__)
static void srf_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void srf_capture(int level, const char *format, va_list args, void *param)
{
	struct srf_log *captured = param;
	captured->count++;
	vsnprintf(srf_current->message, sizeof(srf_current->message), format, args);
	srf_event(SRF_EV_LOG, level, (int64_t)captured->count);
}

static void srf_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		len = size - 1;
	memcpy(dst, src, len);
	dst[len] = '\0';
}

typedef void (*srf_restore)(obs_source_t *, obs_data_array_t *);
typedef void (*srf_get_log)(log_handler_t *, void **);
typedef void (*srf_set_log)(log_handler_t, void *);

/* The out array, by index. */
enum {
	SRF_OUT_COUNT_AFTER = 0,
	SRF_OUT_ORDER,
	SRF_OUT_TARGETS = SRF_OUT_ORDER + SRF_LIST,
	SRF_OUT_PARENTS = SRF_OUT_TARGETS + SRF_LIST,
	SRF_OUT_UNCHANGED = SRF_OUT_PARENTS + SRF_LIST,
	SRF_OUT_UNLOCKED,
	SRF_OUT_LOGS,
	SRF_OUT_HELD,
	SRF_OUT_EVENTS,
	SRF_OUT_OVERFLOW,
	SRF_OUT_HEAD,
	SRF_OUT_COUNT = SRF_OUT_HEAD + SRF_EVENTS * SRF_EVENT_WIDTH,
};

/* Runs one obs_source_restore_filters() through `restore` (the Rust export
 * or a mutant; NULL for the original) on a fresh fixture (see srf_new()), on
 * the parent (NULL when present is false) and the array (NULL when
 * with_array is false). Writes SRF_OUT_COUNT values to out and the last log
 * line to message; false when the fixture could not be built. */
bool oracle_srf_run(srf_restore restore, srf_get_log get_log, srf_set_log set_log, bool present, bool with_array,
		    uint8_t noise, size_t count, size_t item_count, uint32_t items, long dying, int64_t *out,
		    char *message, size_t message_size);
bool oracle_srf_run(srf_restore restore, srf_get_log get_log, srf_set_log set_log, bool present, bool with_array,
		    uint8_t noise, size_t count, size_t item_count, uint32_t items, long dying, int64_t *out,
		    char *message, size_t message_size)
{
	if (count > SRF_SLOTS || item_count > SRF_ITEMS || message_size == 0)
		return false;
	struct srf_fixture *f = srf_new(noise, count, item_count, items, dying);
	if (!f)
		return false;
	obs_source_t *source = present ? &f->parent : NULL;
	obs_data_array_t *array = with_array ? (obs_data_array_t *)(void *)&f->array_marker : NULL;
	struct srf_log captured = {0};
	log_handler_t previous;
	void *param;

	srf_current = f;
	get_log(&previous, &param);
	set_log(srf_capture, &captured);
	if (restore)
		restore(source, array);
	else
		oracle_obs_source_restore_filters(source, array);
	set_log(previous, param);

	memset(out, 0, sizeof(int64_t) * SRF_OUT_COUNT);
	out[SRF_OUT_COUNT_AFTER] = (int64_t)f->parent.filters.num;
	for (size_t i = 0; i < SRF_LIST; i++)
		out[SRF_OUT_ORDER + i] = i < f->parent.filters.num ? srf_id(f->parent.filters.array[i]) : SRF_UNTOUCHED;
	for (size_t i = 0; i < SRF_SLOTS; i++) {
		out[SRF_OUT_TARGETS + i] = srf_link(&f->filters_before[i].filter_target, &f->filters[i].filter_target);
		out[SRF_OUT_PARENTS + i] = srf_link(&f->filters_before[i].filter_parent, &f->filters[i].filter_parent);
	}
	for (size_t i = 0; i < SRF_ITEMS; i++) {
		out[SRF_OUT_TARGETS + SRF_SLOTS + i] =
			srf_link(&f->loaded_before[i].filter_target, &f->loaded[i].filter_target);
		out[SRF_OUT_PARENTS + SRF_SLOTS + i] =
			srf_link(&f->loaded_before[i].filter_parent, &f->loaded[i].filter_parent);
	}

	/* The list may change its array, length and capacity (compared above
	 * by its entries); nothing else of the parent, and of each filter
	 * only its target and parent. */
	bool unchanged = srf_equal_except(&f->parent_before, &f->parent, sizeof(f->parent),
					  offsetof(struct obs_source, filter_mutex), sizeof(f->parent.filter_mutex),
					  offsetof(struct obs_source, filters), sizeof(f->parent.filters));
	for (size_t i = 0; i < SRF_SLOTS; i++)
		unchanged = unchanged && srf_equal_except(&f->filters_before[i], &f->filters[i], sizeof(f->filters[i]),
							  offsetof(struct obs_source, filter_target),
							  sizeof(f->filters[i].filter_target),
							  offsetof(struct obs_source, filter_parent),
							  sizeof(f->filters[i].filter_parent));
	for (size_t i = 0; i < SRF_ITEMS; i++)
		unchanged = unchanged && srf_equal_except(&f->loaded_before[i], &f->loaded[i], sizeof(f->loaded[i]),
							  offsetof(struct obs_source, filter_target),
							  sizeof(f->loaded[i].filter_target),
							  offsetof(struct obs_source, filter_parent),
							  sizeof(f->loaded[i].filter_parent));
	out[SRF_OUT_UNCHANGED] = unchanged;
	out[SRF_OUT_UNLOCKED] = !srf_locked();
	out[SRF_OUT_LOGS] = (int64_t)captured.count;
	out[SRF_OUT_HELD] = f->held;
	out[SRF_OUT_EVENTS] = (int64_t)f->event_count;
	out[SRF_OUT_OVERFLOW] = f->overflow;
	for (size_t i = 0; i < f->event_count; i++)
		memcpy(&out[SRF_OUT_HEAD + i * SRF_EVENT_WIDTH], f->events[i], sizeof(f->events[i]));
	srf_copy(message, message_size, f->message);

	bool ok = true;
	srf_current = NULL;
	if (!out[SRF_OUT_UNLOCKED])
		pthread_mutex_unlock(&f->parent.filter_mutex);
	srf_free(f, &ok);
	return ok;
}

/* The sizes the Rust side mirrors: 0 the out array, 1 the filter slots,
 * 2 the item slots, 3 the event width, 4 the item name selectors. */
size_t oracle_srf_layout(size_t which);
size_t oracle_srf_layout(size_t which)
{
	switch (which) {
	case 0:
		return SRF_OUT_COUNT;
	case 1:
		return SRF_SLOTS;
	case 2:
		return SRF_ITEMS;
	case 3:
		return SRF_EVENT_WIDTH;
	case 4:
		return SRF_ITEM_NAMES;
	default:
		return 0;
	}
}
