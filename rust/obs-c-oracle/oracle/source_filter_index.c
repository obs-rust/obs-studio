/* Original getter and real private-layout adapters for Tier 3. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_filter_get_index oracle_obs_source_filter_get_index
#include "obs-source-filter-index.c.h"
#undef obs_source_filter_get_index
#include "source_filter_index_fields.inc"

struct filter_index_fixture {
	struct obs_source source;
	struct obs_source tokens[8];
	obs_source_t *items[64];
	obs_source_t *before[64];
	size_t count;
};
void *oracle_filter_index_new(const unsigned char *ids, size_t count);
void *oracle_filter_index_new(const unsigned char *ids, size_t count)
{
	if (count > 64)
		return NULL;
	struct filter_index_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	if (pthread_mutex_init(&f->source.filter_mutex, NULL) != 0) {
		free(f);
		return NULL;
	}
	for (size_t i = 0; i < count; i++)
		f->items[i] = &f->tokens[ids[i] % 8];
	memcpy(f->before, f->items, sizeof(f->items));
	f->count = count;
	f->source.filters.array = f->items;
	f->source.filters.num = count;
	f->source.filters.capacity = 64;
	return f;
}
void *oracle_filter_index_token(void *handle, unsigned id);
void *oracle_filter_index_token(void *handle, unsigned id)
{
	struct filter_index_fixture *f = handle;
	return id < 8 ? &f->tokens[id] : &f->source;
}
bool oracle_filter_index_unchanged(void *handle);
bool oracle_filter_index_unchanged(void *handle)
{
	struct filter_index_fixture *f = handle;
	int locked = pthread_mutex_trylock(&f->source.filter_mutex);
	if (locked != 0)
		return false;
	pthread_mutex_unlock(&f->source.filter_mutex);
	return f->source.filters.array == f->items && f->source.filters.num == f->count &&
	       f->source.filters.capacity == 64 && memcmp(f->items, f->before, sizeof(f->items)) == 0;
}
bool oracle_filter_index_free(void *handle);
bool oracle_filter_index_free(void *handle)
{
	struct filter_index_fixture *f = handle;
	int result = pthread_mutex_destroy(&f->source.filter_mutex);
	free(f);
	return result == 0;
}

struct filter_index_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void filter_index_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void filter_index_capture(int level, const char *format, va_list args, void *param)
{
	struct filter_index_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
typedef int (*filter_index_getter)(obs_source_t *, obs_source_t *);
typedef void (*filter_index_get_log)(log_handler_t *, void **);
typedef void (*filter_index_set_log)(log_handler_t, void *);
bool oracle_filter_index_logs(filter_index_getter getter, filter_index_get_log get, filter_index_set_log set);
bool oracle_filter_index_logs(filter_index_getter getter, filter_index_get_log get, filter_index_set_log set)
{
	log_handler_t old;
	void *param;
	get(&old, &param);
	struct obs_source source = {0};
	obs_source_t *sources[] = {NULL, NULL, &source};
	obs_source_t *filters[] = {&source, NULL, NULL};
	bool ok = true;
	for (size_t i = 0; i < 3; i++) {
		struct filter_index_log r = {0}, c = {0};
		set(filter_index_capture, &r);
		int rv = getter(sources[i], filters[i]);
		set(filter_index_capture, &c);
		int cv = oracle_obs_source_filter_get_index(sources[i], filters[i]);
		const char *expected = i < 2 ? "obs_source_filter_get_index: Null 'source' parameter"
					     : "obs_source_filter_get_index: Null 'filter' parameter";
		ok = ok && rv == -1 && cv == -1 && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
		     c.level == LOG_DEBUG && strcmp(r.message, expected) == 0 && strcmp(r.message, c.message) == 0;
	}
	set(old, param);
	return ok;
}
