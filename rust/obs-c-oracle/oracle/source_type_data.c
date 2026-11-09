/* Extracted original C and discovered adapter over the real private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_type_data oracle_obs_source_get_type_data
#include "obs-source-type-data.c.h"
#undef obs_source_get_type_data
#include "source_type_data_fields.inc"

struct source_type_data_fixture {
	struct obs_source source;
	struct obs_source before;
	/* Borrowed addresses only; align each token without a C library alignment typedef. */
	_Alignas(long double) unsigned char tokens[2][sizeof(long double)];
};

void *oracle_source_type_data_new(bool present, bool same, uint32_t flags);
void *oracle_source_type_data_new(bool present, bool same, uint32_t flags)
{
	struct source_type_data_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.info.type_data = present ? (void *)&fixture->tokens[same ? 0 : 1] : NULL;
		fixture->source.context.data = &fixture->tokens[0];
		fixture->source.info.output_flags = flags;
		memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

void oracle_source_type_data_tokens(void *handle, void **tokens);
void oracle_source_type_data_tokens(void *handle, void **tokens)
{
	struct source_type_data_fixture *fixture = handle;
	tokens[0] = &fixture->tokens[0];
	tokens[1] = &fixture->tokens[1];
}

bool oracle_source_type_data_unchanged(const void *handle);
bool oracle_source_type_data_unchanged(const void *handle)
{
	const struct source_type_data_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->source, sizeof(fixture->source)) == 0;
}

void oracle_source_type_data_free(void *handle);
void oracle_source_type_data_free(void *handle)
{
	free(handle);
}

bool oracle_source_type_data_addresses(const obs_source_t *source);
bool oracle_source_type_data_addresses(const obs_source_t *source)
{
	struct obs_source_type_data_fields fields = obs_source_type_data_fields(source);
	return fields.data == &source->info.type_data;
}

void oracle_source_type_data_layout(size_t *layout);
void oracle_source_type_data_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_type_data_fields);
	layout[1] = _Alignof(struct obs_source_type_data_fields);
	layout[2] = offsetof(struct obs_source_type_data_fields, data);
	layout[3] = sizeof(void *);
	layout[4] = _Alignof(void *);
}

static void *source_type_data_original(obs_source_t *source)
{
	return oracle_obs_source_get_type_data(source);
}

struct source_type_data_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_type_data_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_type_data_capture(int level, const char *format, va_list args, void *param)
{
	struct source_type_data_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void *(*source_type_data_getter)(obs_source_t *);
typedef void (*source_type_data_get_log)(log_handler_t *, void **);
typedef void (*source_type_data_set_log)(log_handler_t, void *);
bool oracle_source_type_data_logs(source_type_data_getter getter, source_type_data_get_log get_log,
				  source_type_data_set_log set_log);
bool oracle_source_type_data_logs(source_type_data_getter getter, source_type_data_get_log get_log,
				  source_type_data_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	source_type_data_getter rust[] = {getter};
	source_type_data_getter original[] = {source_type_data_original};
	const char *expected[] = {"obs_source_get_type_data: Null 'source' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 1; i++) {
		struct source_type_data_log r = {0}, c = {0};
		set_log(source_type_data_capture, &r);
		void *rust_value = rust[i](NULL);
		set_log(source_type_data_capture, &c);
		void *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
