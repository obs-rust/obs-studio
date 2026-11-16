/* Original C getter and production adapter over the real private source layout. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_name oracle_obs_source_get_name
#include "obs-source-get-name.c.h"
#undef obs_source_get_name
#include "source-name-fields.inc"

struct source_name_fixture {
	struct obs_source source;
	struct obs_source before;
};

void *oracle_source_name_new(char *name, bool private_source, uint32_t flags);
void *oracle_source_name_new(char *name, bool private_source, uint32_t flags)
{
	struct source_name_fixture *f = calloc(1, sizeof(*f));
	if (f) {
		f->source.context.name = name;
		f->source.context.private = private_source;
		f->source.info.output_flags = flags;
		memcpy(&f->before, &f->source, sizeof(f->source));
	}
	return f;
}

void oracle_source_name_set(void *handle, char *name);
void oracle_source_name_set(void *handle, char *name)
{
	struct source_name_fixture *f = handle;
	f->source.context.name = name;
	memcpy(&f->before, &f->source, sizeof(f->source));
}

bool oracle_source_name_unchanged(const void *handle);
bool oracle_source_name_unchanged(const void *handle)
{
	const struct source_name_fixture *f = handle;
	return memcmp(&f->source, &f->before, sizeof(f->source)) == 0;
}

void oracle_source_name_free(void *handle);
void oracle_source_name_free(void *handle)
{
	free(handle);
}

/* Existing missing-file tests serialize access to this real-layout fixture. */
static struct obs_source source_name_missing_source;
void *oracle_source_name_missing_source(void);
void *oracle_source_name_missing_source(void)
{
	return &source_name_missing_source;
}
void oracle_source_name_missing_set(char *name);
void oracle_source_name_missing_set(char *name)
{
	source_name_missing_source.context.name = name;
}

static const char *source_name_original_proc(const obs_source_t *source)
{
	return oracle_obs_source_get_name(source);
}

struct source_name_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_name_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_name_capture(int level, const char *format, va_list args, void *param)
{
	struct source_name_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef const char *(*source_name_getter)(const obs_source_t *);
typedef void (*source_name_get_log)(log_handler_t *, void **);
typedef void (*source_name_set_log)(log_handler_t, void *);
bool oracle_source_name_logs(source_name_getter proc, source_name_get_log get_log, source_name_set_log set_log);
bool oracle_source_name_logs(source_name_getter proc, source_name_get_log get_log, source_name_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	source_name_getter rust[] = {proc};
	source_name_getter original[] = {source_name_original_proc};
	const char *expected[] = {"obs_source_get_name: Null 'source' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 1; i++) {
		struct source_name_log r = {0}, c = {0};
		set_log(source_name_capture, &r);
		const char *rust_value = rust[i](NULL);
		set_log(source_name_capture, &c);
		const char *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
