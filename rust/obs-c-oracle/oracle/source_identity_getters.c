/* Original extracted getters and discovered field adapter over real C layouts. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_get_uuid oracle_obs_source_get_uuid
#define obs_source_get_unversioned_id oracle_obs_source_get_unversioned_id
#include "obs-source-identity-getters.c.h"
#undef obs_source_get_unversioned_id
#undef obs_source_get_uuid
#include "source_identity_getter_fields.inc"

struct source_identity_fixture {
	struct obs_source source;
	struct obs_source before;
};

void *oracle_source_identity_new(const char *uuid, const char *unversioned_id, uint32_t flags);
void *oracle_source_identity_new(const char *uuid, const char *unversioned_id, uint32_t flags)
{
	struct source_identity_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.context.uuid = uuid;
		fixture->source.info.unversioned_id = unversioned_id;
		fixture->source.info.output_flags = flags;
		memcpy(&fixture->before, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

bool oracle_source_identity_unchanged(const void *handle);
bool oracle_source_identity_unchanged(const void *handle)
{
	const struct source_identity_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->source, sizeof(fixture->source)) == 0;
}

void oracle_source_identity_free(void *handle);
void oracle_source_identity_free(void *handle)
{
	free(handle);
}

bool oracle_source_identity_addresses(const obs_source_t *source);
bool oracle_source_identity_addresses(const obs_source_t *source)
{
	struct obs_source_identity_getter_fields fields = obs_source_identity_getter_fields(source);
	return fields.uuid == &source->context.uuid && fields.unversioned_id == &source->info.unversioned_id;
}

void oracle_source_identity_layout(size_t *layout);
void oracle_source_identity_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_identity_getter_fields);
	layout[1] = _Alignof(struct obs_source_identity_getter_fields);
	layout[2] = offsetof(struct obs_source_identity_getter_fields, uuid);
	layout[3] = offsetof(struct obs_source_identity_getter_fields, unversioned_id);
	layout[4] = sizeof(((struct obs_source *)0)->context.uuid);
	layout[5] = _Alignof(const char *);
}

struct source_identity_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_identity_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_identity_capture(int level, const char *format, va_list args, void *param)
{
	struct source_identity_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef const char *(*source_identity_getter)(const obs_source_t *);
typedef void (*source_identity_get_log)(log_handler_t *, void **);
typedef void (*source_identity_set_log)(log_handler_t, void *);
bool oracle_source_identity_logs(source_identity_getter uuid, source_identity_getter unversioned_id,
				 source_identity_get_log get_log, source_identity_set_log set_log);
bool oracle_source_identity_logs(source_identity_getter uuid, source_identity_getter unversioned_id,
				 source_identity_get_log get_log, source_identity_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	source_identity_getter rust[] = {uuid, unversioned_id};
	source_identity_getter original[] = {oracle_obs_source_get_uuid, oracle_obs_source_get_unversioned_id};
	const char *expected[] = {"obs_source_get_uuid: Null 'source' parameter",
				  "obs_source_get_unversioned_id: Null 'source' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		struct source_identity_log r = {0}, c = {0};
		set_log(source_identity_capture, &r);
		const char *rust_value = rust[i](NULL);
		set_log(source_identity_capture, &c);
		const char *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
