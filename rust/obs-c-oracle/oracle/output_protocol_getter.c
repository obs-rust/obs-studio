/* Original extracted getters and discovered field adapter over real C layouts. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_protocols oracle_obs_output_get_protocols
#include "obs-output-protocol-getter.c.h"
#undef obs_output_get_protocols
#include "output_protocol_getter_fields.inc"

struct output_protocol_getter_fixture {
	struct obs_output output;
	struct obs_output before;
};

void *oracle_output_protocol_getter_new(const char *protocols, uint32_t flags);
void *oracle_output_protocol_getter_new(const char *protocols, uint32_t flags)
{
	struct output_protocol_getter_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.info.protocols = protocols;
		fixture->output.info.flags = flags;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}

bool oracle_output_protocol_getter_unchanged(const void *handle);
bool oracle_output_protocol_getter_unchanged(const void *handle)
{
	const struct output_protocol_getter_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}

void oracle_output_protocol_getter_free(void *handle);
void oracle_output_protocol_getter_free(void *handle)
{
	free(handle);
}

bool oracle_output_protocol_getter_addresses(const obs_output_t *output);
bool oracle_output_protocol_getter_addresses(const obs_output_t *output)
{
	struct obs_output_protocol_getter_fields fields = obs_output_protocol_getter_fields(output);
	return fields.flags == &output->info.flags && fields.protocols == &output->info.protocols;
}

void oracle_output_protocol_getter_layout(size_t *layout);
void oracle_output_protocol_getter_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_protocol_getter_fields);
	layout[1] = _Alignof(struct obs_output_protocol_getter_fields);
	layout[2] = offsetof(struct obs_output_protocol_getter_fields, flags);
	layout[3] = offsetof(struct obs_output_protocol_getter_fields, protocols);
	layout[4] = sizeof(((struct obs_output *)0)->info.flags);
	layout[5] = OBS_OUTPUT_SERVICE;
}

struct output_protocol_getter_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_protocol_getter_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_protocol_getter_capture(int level, const char *format, va_list args, void *param)
{
	struct output_protocol_getter_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);

}

typedef const char *(*output_protocol_getter_getter)(const obs_output_t *);
typedef void (*output_protocol_getter_get_log)(log_handler_t *, void **);
typedef void (*output_protocol_getter_set_log)(log_handler_t, void *);
bool oracle_output_protocol_getter_logs(output_protocol_getter_getter getter, output_protocol_getter_get_log get_log,
					output_protocol_getter_set_log set_log);
bool oracle_output_protocol_getter_logs(output_protocol_getter_getter getter, output_protocol_getter_get_log get_log,
					output_protocol_getter_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	output_protocol_getter_getter rust[] = {getter};
	output_protocol_getter_getter original[] = {oracle_obs_output_get_protocols};
	const char *expected[] = {"obs_output_get_protocols: Null 'output' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 1; i++) {
		struct output_protocol_getter_log r = {0}, c = {0};
		set_log(output_protocol_getter_capture, &r);
		const char *rust_value = rust[i](NULL);
		set_log(output_protocol_getter_capture, &c);
		const char *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
