/* Original extracted getters and discovered adapter over real C layouts. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_id oracle_obs_output_get_id
#define obs_output_get_type_data oracle_obs_output_get_type_data
#include "obs-output-identity-getters.c.h"
#undef obs_output_get_type_data
#undef obs_output_get_id
#include "output_identity_fields.inc"

struct output_identity_fixture {
	struct obs_output output;
	struct obs_output before;
};

void *oracle_output_identity_new(const char *id, void *data, uint32_t flags);
void *oracle_output_identity_new(const char *id, void *data, uint32_t flags)
{
	struct output_identity_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.info.id = id;
		fixture->output.info.type_data = data;
		fixture->output.info.flags = flags;
		fixture->output.context.name = "different-context-name";
		fixture->output.context.data = &fixture->output;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}

bool oracle_output_identity_unchanged(const void *handle);
bool oracle_output_identity_unchanged(const void *handle)
{
	const struct output_identity_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}

void oracle_output_identity_free(void *handle);
void oracle_output_identity_free(void *handle)
{
	free(handle);
}

bool oracle_output_identity_addresses(const obs_output_t *output);
bool oracle_output_identity_addresses(const obs_output_t *output)
{
	struct obs_output_identity_fields fields = obs_output_identity_fields(output);
	return fields.id == &output->info.id && fields.type_data == &output->info.type_data;
}

void oracle_output_identity_layout(size_t *layout);
void oracle_output_identity_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_identity_fields);
	layout[1] = _Alignof(struct obs_output_identity_fields);
	layout[2] = offsetof(struct obs_output_identity_fields, id);
	layout[3] = offsetof(struct obs_output_identity_fields, type_data);
}

struct output_identity_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_identity_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_identity_capture(int level, const char *format, va_list args, void *param)
{
	struct output_identity_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef const char *(*output_identity_id_getter)(const obs_output_t *);
typedef void *(*output_identity_data_getter)(obs_output_t *);
typedef void (*output_identity_get_log)(log_handler_t *, void **);
typedef void (*output_identity_set_log)(log_handler_t, void *);

bool oracle_output_identity_logs(output_identity_id_getter id, output_identity_data_getter data,
				 output_identity_get_log get_log, output_identity_set_log set_log);
bool oracle_output_identity_logs(output_identity_id_getter id, output_identity_data_getter data,
				 output_identity_get_log get_log, output_identity_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		struct output_identity_log r = {0}, c = {0};
		set_log(output_identity_capture, &r);
		bool rust_null = i == 0 ? id(NULL) == NULL : data(NULL) == NULL;
		set_log(output_identity_capture, &c);
		bool c_null = i == 0 ? oracle_obs_output_get_id(NULL) == NULL
				     : oracle_obs_output_get_type_data(NULL) == NULL;
		const char *expected = i == 0 ? "obs_output_get_id: Null 'output' parameter"
					      : "obs_output_get_type_data: Null 'output' parameter";
		equal = equal && rust_null && c_null && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected) == 0 && strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
