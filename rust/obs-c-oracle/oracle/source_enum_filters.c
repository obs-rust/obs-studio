/* Original obs_source_enum_filters() over the real private layout, and the
 * production field adapters the Rust shim reads through
 * (obs-source-enum-filters-fields.c.h, defined once, here). */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_enum_filters oracle_obs_source_enum_filters
#include "libobs/build/libobs.pch.h"
#include "source-enum-filters.inc"
#undef obs_source_enum_filters
#include "source_enum_filters_fields.inc"

#define ENUM_FILTERS 8

struct enum_fixture {
	struct obs_source parent;
	struct obs_source filters[ENUM_FILTERS];
	obs_source_t *list[ENUM_FILTERS];
	obs_source_t *other[ENUM_FILTERS];
	struct obs_source parent_before;
	struct obs_source filters_before[ENUM_FILTERS];
};

static struct enum_fixture *enum_new(uint8_t noise, size_t count)
{
	struct enum_fixture *f = malloc(sizeof(*f));
	if (!f)
		return NULL;
	memset(f, noise, sizeof(*f));
	if (pthread_mutex_init(&f->parent.filter_mutex, NULL) != 0) {
		free(f);
		return NULL;
	}
	for (size_t i = 0; i < ENUM_FILTERS; i++) {
		f->list[i] = &f->filters[i];
		f->other[i] = &f->filters[ENUM_FILTERS - 1 - i];
	}
	f->parent.filters.array = f->list;
	f->parent.filters.num = count;
	f->parent.filters.capacity = ENUM_FILTERS;
	memcpy(&f->parent_before, &f->parent, sizeof(f->parent));
	memcpy(f->filters_before, f->filters, sizeof(f->filters));
	return f;
}

/* The parent's bytes outside its mutex and every filter's bytes are as they
 * were. */
static bool enum_unchanged(struct enum_fixture *f)
{
	const size_t at = offsetof(struct obs_source, filter_mutex);
	const size_t end = at + sizeof(f->parent.filter_mutex);
	return memcmp(&f->parent_before, &f->parent, at) == 0 &&
	       memcmp((const unsigned char *)&f->parent_before + end, (const unsigned char *)&f->parent + end,
		      sizeof(f->parent) - end) == 0 &&
	       memcmp(f->filters_before, f->filters, sizeof(f->filters)) == 0;
}

static bool enum_unlocked(struct enum_fixture *f)
{
	if (pthread_mutex_trylock(&f->parent.filter_mutex) != 0)
		return false;
	pthread_mutex_unlock(&f->parent.filter_mutex);
	return true;
}

static bool enum_free(struct enum_fixture *f)
{
	bool ok = pthread_mutex_destroy(&f->parent.filter_mutex) == 0;
	free(f);
	return ok;
}

static long enum_index(struct enum_fixture *f, obs_source_t *filter)
{
	if (!filter)
		return -1;
	for (size_t i = 0; i < ENUM_FILTERS; i++) {
		if (filter == &f->filters[i])
			return (long)i;
	}
	return -2;
}

static void enum_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		len = size - 1;
	memcpy(dst, src, len);
	dst[len] = '\0';
}

struct enum_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void enum_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void enum_capture(int level, const char *format, va_list args, void *param)
{
	struct enum_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*enum_query)(obs_source_t *, obs_source_enum_proc_t, void *);
typedef void (*enum_get_log)(log_handler_t *, void **);
typedef void (*enum_set_log)(log_handler_t, void *);

enum { ENUM_MODE_NONE, ENUM_MODE_SWAP_ARRAY, ENUM_MODE_SHRINK, ENUM_MODE_REPLACE_NEXT, ENUM_MODES };

struct enum_visit {
	struct enum_fixture *f;
	int mode;
	size_t calls;
	long children[2 * ENUM_FILTERS];
	bool locked[2 * ENUM_FILTERS];
	bool arguments_ok;
};

static void enum_record(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct enum_visit *v = param;
	struct enum_fixture *f = v->f;
	if (v->calls >= 2 * ENUM_FILTERS) {
		v->arguments_ok = false;
		return;
	}
	v->arguments_ok = v->arguments_ok && parent == &f->parent;
	v->children[v->calls] = enum_index(f, child);
	int got = pthread_mutex_trylock(&f->parent.filter_mutex);
	if (got == 0)
		pthread_mutex_unlock(&f->parent.filter_mutex);
	v->locked[v->calls] = got != 0;
	if (v->calls == 0) {
		switch (v->mode) {
		case ENUM_MODE_SWAP_ARRAY:
			f->parent.filters.array = f->other;
			break;
		case ENUM_MODE_SHRINK:
			f->parent.filters.num = 1;
			break;
		case ENUM_MODE_REPLACE_NEXT:
			if (f->parent.filters.num >= 2)
				f->parent.filters.array[f->parent.filters.num - 2] = &f->filters[ENUM_FILTERS - 1];
			break;
		default:
			break;
		}
	}
	v->calls++;
}

/* The given enumeration and the original each run on their own fixture
 * with the same inputs (NULL source when present is false, NULL callback
 * when callback is false). Both must call back alike (the same filter
 * indexes, the parent and param passed through, the mutex held), log alike
 * (once for a NULL argument, never otherwise), leave the mutex free and,
 * when the callback changes nothing, every byte as it was. The original's
 * call count goes to *calls and its log line to message. */
bool oracle_enum_filters_check(enum_query query, enum_get_log get_log, enum_set_log set_log, bool present,
			       bool callback, uint8_t noise, size_t count, int mode, size_t *calls, char *message,
			       size_t message_size);
bool oracle_enum_filters_check(enum_query query, enum_get_log get_log, enum_set_log set_log, bool present,
			       bool callback, uint8_t noise, size_t count, int mode, size_t *calls, char *message,
			       size_t message_size)
{
	if (count > ENUM_FILTERS || mode < 0 || mode >= ENUM_MODES || message_size == 0)
		return false;
	enum_query queries[2] = {query, oracle_obs_source_enum_filters};
	struct enum_visit visits[2];
	struct enum_log logs[2];
	log_handler_t previous;
	void *param;
	bool equal = true;
	get_log(&previous, &param);
	for (size_t side = 0; side < 2; side++) {
		struct enum_fixture *f = enum_new(noise, count);
		if (!f)
			return false;
		memset(&visits[side], 0, sizeof(visits[side]));
		visits[side].f = f;
		visits[side].mode = mode;
		visits[side].arguments_ok = true;
		memset(&logs[side], 0, sizeof(logs[side]));
		logs[side].level = -1;
		set_log(enum_capture, &logs[side]);
		queries[side](present ? &f->parent : NULL, callback ? enum_record : NULL, &visits[side]);
		set_log(previous, param);
		equal = equal && visits[side].arguments_ok && enum_unlocked(f);
		if (mode == ENUM_MODE_NONE)
			equal = equal && enum_unchanged(f);
		for (size_t i = 0; i < visits[side].calls; i++)
			equal = equal && visits[side].locked[i];
		equal = enum_free(f) && equal;
	}
	const bool logs_expected = !present || !callback;
	equal = equal && visits[0].calls == visits[1].calls &&
		memcmp(visits[0].children, visits[1].children, visits[1].calls * sizeof(long)) == 0 &&
		logs[0].count == logs[1].count && logs[1].count == (logs_expected ? 1u : 0u) &&
		logs[0].level == logs[1].level && strcmp(logs[0].message, logs[1].message) == 0;
	*calls = visits[1].calls;
	enum_copy(message, message_size, logs[1].message);
	return equal;
}
