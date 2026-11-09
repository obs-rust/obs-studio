/* Tier 3 uses the original getter and adapter over the real private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_filter_count oracle_obs_source_filter_count
#include "obs-source-filter-count.c.h"
#undef obs_source_filter_count
#include "source_filter_count_fields.inc"

struct filter_count_fixture {
	struct obs_source source;
	struct obs_source before;
};

void *oracle_filter_count_new(size_t count);
void *oracle_filter_count_new(size_t count)
{
	struct filter_count_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.filters.num = count;
		memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

void oracle_filter_count_free(void *fixture);
void oracle_filter_count_free(void *fixture)
{
	free(fixture);
}

bool oracle_filter_count_unchanged(const void *handle);
bool oracle_filter_count_unchanged(const void *handle)
{
	const struct filter_count_fixture *fixture = handle;
	return memcmp(&fixture->source, &fixture->before, sizeof(fixture->source)) == 0;
}

bool oracle_filter_count_address(const obs_source_t *source);
bool oracle_filter_count_address(const obs_source_t *source)
{
	return obs_source_filter_count_fields(source).count == &source->filters.num;
}

void oracle_filter_count_layout(size_t *layout);
void oracle_filter_count_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_filter_count_fields);
	layout[1] = _Alignof(struct obs_source_filter_count_fields);
	layout[2] = offsetof(struct obs_source_filter_count_fields, count);
	layout[3] = sizeof(size_t);
	layout[4] = _Alignof(size_t);
}

struct filter_count_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void filter_count_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void filter_count_capture(int level, const char *format, va_list args, void *param)
{
	struct filter_count_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef size_t (*filter_count_getter)(const obs_source_t *);
typedef void (*filter_count_get_log)(log_handler_t *, void **);
typedef void (*filter_count_set_log)(log_handler_t, void *);
bool oracle_filter_count_logs(filter_count_getter getter, filter_count_get_log get, filter_count_set_log set);
bool oracle_filter_count_logs(filter_count_getter getter, filter_count_get_log get, filter_count_set_log set)
{
	log_handler_t old;
	void *param;
	get(&old, &param);
	struct filter_count_log r = {0}, c = {0};
	set(filter_count_capture, &r);
	size_t rust_value = getter(NULL);
	set(filter_count_capture, &c);
	size_t c_value = oracle_obs_source_filter_count(NULL);
	set(old, param);
	return rust_value == 0 && c_value == 0 && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG && strcmp(r.message, "obs_source_filter_count: Null 'source' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
