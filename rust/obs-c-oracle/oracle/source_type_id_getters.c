/* Original extracted getters, over the real private source layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_type oracle_obs_source_get_type
#define obs_source_get_id oracle_obs_source_get_id
#include "obs-source-type-id-getters.c.h"
#undef obs_source_get_id
#undef obs_source_get_type
#include "source_type_id_fields.inc"

struct source_type_id_fixture {
	struct obs_source source;
	struct obs_source before;
	char name[64];
};

void *oracle_source_type_id_new(unsigned type, const char *id, const char *name);
void *oracle_source_type_id_new(unsigned type, const char *id, const char *name)
{
	struct source_type_id_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.info.type = (enum obs_source_type)type;
		fixture->source.info.id = id;
		snprintf(fixture->name, sizeof(fixture->name), "%s", name);
		fixture->source.context.name = fixture->name;
		memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

void oracle_source_type_id_free(void *fixture);
void oracle_source_type_id_free(void *fixture)
{
	free(fixture);
}

void oracle_source_type_id_rename(void *handle, const char *name);
void oracle_source_type_id_rename(void *handle, const char *name)
{
	struct source_type_id_fixture *fixture = handle;
	snprintf(fixture->name, sizeof(fixture->name), "%s", name);
	memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
}

bool oracle_source_type_id_unchanged(const void *handle);
bool oracle_source_type_id_unchanged(const void *handle)
{
	const struct source_type_id_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->source, sizeof(fixture->source)) == 0;
}

bool oracle_source_type_id_addresses(const obs_source_t *source);
bool oracle_source_type_id_addresses(const obs_source_t *source)
{
	return obs_source_type_id_type_field(source) == &source->info.type &&
	       obs_source_type_id_id_field(source) == &source->info.id;
}

void oracle_source_type_id_layout(size_t *layout);
void oracle_source_type_id_layout(size_t *layout)
{
	layout[0] = sizeof(enum obs_source_type);
	layout[1] = _Alignof(enum obs_source_type);
	layout[2] = OBS_SOURCE_TYPE_INPUT;
	layout[3] = OBS_SOURCE_TYPE_FILTER;
	layout[4] = OBS_SOURCE_TYPE_TRANSITION;
	layout[5] = OBS_SOURCE_TYPE_SCENE;
}

struct source_type_id_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_type_id_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_type_id_capture(int level, const char *format, va_list args, void *param)
{
	struct source_type_id_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef const char *(*source_type_id_getter)(const obs_source_t *);
typedef enum obs_source_type (*source_type_id_type_getter)(const obs_source_t *);
typedef void (*source_type_id_get_log)(log_handler_t *, void **);
typedef void (*source_type_id_set_log)(log_handler_t, void *);
bool oracle_source_type_id_logs(source_type_id_type_getter type_getter, source_type_id_getter getter,
				source_type_id_get_log get_log, source_type_id_set_log set_log);
bool oracle_source_type_id_logs(source_type_id_type_getter type_getter, source_type_id_getter getter,
				source_type_id_get_log get_log, source_type_id_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	source_type_id_getter rust[] = {getter};
	source_type_id_getter original[] = {oracle_obs_source_get_id};
	const char *expected[] = {"obs_source_get_id: Null 'source' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 1; i++) {
		struct source_type_id_log r = {0}, c = {0};
		set_log(source_type_id_capture, &r);
		const char *rust_value = rust[i](NULL);
		set_log(source_type_id_capture, &c);
		const char *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	struct source_type_id_log r = {0}, c = {0};
	set_log(source_type_id_capture, &r);
	enum obs_source_type rust_type = type_getter(NULL);
	set_log(source_type_id_capture, &c);
	enum obs_source_type c_type = oracle_obs_source_get_type(NULL);
	equal = equal && rust_type == OBS_SOURCE_TYPE_INPUT && c_type == OBS_SOURCE_TYPE_INPUT && r.count == 1 &&
		c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
		strcmp(r.message, "obs_source_get_type: Null 'source' parameter") == 0 &&
		strcmp(r.message, c.message) == 0;
	set_log(old_handler, old_param);
	return equal;
}
