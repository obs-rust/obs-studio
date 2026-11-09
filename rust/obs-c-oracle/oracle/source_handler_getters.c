/* Extracted original C and discovered adapter over the real private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_proc_handler oracle_obs_source_get_proc_handler
#include "obs-source-handler-getters.c.h"
#undef obs_source_get_proc_handler
#include "source_handler_getters_fields.inc"

struct source_handlers_fixture {
	struct obs_source source;
	struct obs_source before;
	/* Borrowed addresses only; align each token without a C library alignment typedef. */
	_Alignas(long double) unsigned char tokens[2][sizeof(long double)];
};

void *oracle_source_handlers_new(bool signal, bool proc, bool same, uint32_t flags);
void *oracle_source_handlers_new(bool signal, bool proc, bool same, uint32_t flags)
{
	struct source_handlers_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.context.signals = signal ? (void *)&fixture->tokens[0] : NULL;
		fixture->source.context.procs = proc ? (void *)&fixture->tokens[same ? 0 : 1] : NULL;
		fixture->source.info.output_flags = flags;
		memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

void oracle_source_handlers_tokens(void *handle, void **tokens);
void oracle_source_handlers_tokens(void *handle, void **tokens)
{
	struct source_handlers_fixture *fixture = handle;
	tokens[0] = &fixture->tokens[0];
	tokens[1] = &fixture->tokens[1];
}

bool oracle_source_handlers_unchanged(const void *handle);
bool oracle_source_handlers_unchanged(const void *handle)
{
	const struct source_handlers_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->source, sizeof(fixture->source)) == 0;
}

void oracle_source_handlers_free(void *handle);
void oracle_source_handlers_free(void *handle)
{
	free(handle);
}

bool oracle_source_handlers_addresses(const obs_source_t *source);
bool oracle_source_handlers_addresses(const obs_source_t *source)
{
	struct obs_source_handler_getters_fields fields = obs_source_handler_getters_fields(source);
	return fields.proc == &source->context.procs;
}

void oracle_source_handlers_layout(size_t *layout);
void oracle_source_handlers_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_handler_getters_fields);
	layout[1] = _Alignof(struct obs_source_handler_getters_fields);
	layout[2] = offsetof(struct obs_source_handler_getters_fields, proc);
	layout[3] = sizeof(proc_handler_t *);
	layout[4] = _Alignof(proc_handler_t *);
}

static void *source_handlers_original_proc(const obs_source_t *source)
{
	return oracle_obs_source_get_proc_handler(source);
}

struct source_handlers_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_handlers_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_handlers_capture(int level, const char *format, va_list args, void *param)
{
	struct source_handlers_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void *(*source_handlers_getter)(const obs_source_t *);
typedef void (*source_handlers_get_log)(log_handler_t *, void **);
typedef void (*source_handlers_set_log)(log_handler_t, void *);
bool oracle_source_handlers_logs(source_handlers_getter proc, source_handlers_get_log get_log,
				 source_handlers_set_log set_log);
bool oracle_source_handlers_logs(source_handlers_getter proc, source_handlers_get_log get_log,
				 source_handlers_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	source_handlers_getter rust[] = {proc};
	source_handlers_getter original[] = {source_handlers_original_proc};
	const char *expected[] = {"obs_source_get_proc_handler: Null 'source' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 1; i++) {
		struct source_handlers_log r = {0}, c = {0};
		set_log(source_handlers_capture, &r);
		void *rust_value = rust[i](NULL);
		set_log(source_handlers_capture, &c);
		void *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
