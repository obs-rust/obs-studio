/* Original C and discovered adapter over the real private output layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_service oracle_obs_output_get_service
#include "obs-output-service-getter.c.h"
#undef obs_output_get_service
#include "output_service_getter_fields.inc"

struct output_service_fixture {
	struct obs_output output;
	struct obs_output before;
	struct obs_service services[2];
};

void *oracle_output_service_new(unsigned which, uint32_t flags);
void *oracle_output_service_new(unsigned which, uint32_t flags)
{
	struct output_service_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.service = which ? &fixture->services[(which - 1) % 2] : NULL;
		fixture->output.info.flags = flags;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}

void *oracle_output_service_expected(const void *handle);
void *oracle_output_service_expected(const void *handle)
{
	const struct output_service_fixture *fixture = handle;
	return fixture->before.service;
}

bool oracle_output_service_unchanged(const void *handle);
bool oracle_output_service_unchanged(const void *handle)
{
	const struct output_service_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}

void oracle_output_service_free(void *handle);
void oracle_output_service_free(void *handle)
{
	free(handle);
}

bool oracle_output_service_addresses(const obs_output_t *output);
bool oracle_output_service_addresses(const obs_output_t *output)
{
	return obs_output_service_getter_fields(output).service == &output->service;
}

void oracle_output_service_layout(size_t *layout);
void oracle_output_service_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_service_getter_fields);
	layout[1] = _Alignof(struct obs_output_service_getter_fields);
	layout[2] = offsetof(struct obs_output_service_getter_fields, service);
	layout[3] = sizeof(obs_service_t *);
	layout[4] = _Alignof(obs_service_t *);
}

struct output_service_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_service_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_service_capture(int level, const char *format, va_list args, void *param)
{
	struct output_service_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef void *(*output_service_getter)(const obs_output_t *);
typedef void (*output_service_get_log)(log_handler_t *, void **);
typedef void (*output_service_set_log)(log_handler_t, void *);
bool oracle_output_service_logs(output_service_getter getter, output_service_get_log get_log,
				output_service_set_log set_log);
bool oracle_output_service_logs(output_service_getter getter, output_service_get_log get_log,
				output_service_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_service_log r = {0}, c = {0};
	set_log(output_service_capture, &r);
	void *rust_value = getter(NULL);
	set_log(output_service_capture, &c);
	void *c_value = oracle_obs_output_get_service(NULL);
	set_log(old_handler, old_param);
	return !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG && strcmp(r.message, "obs_output_get_service: Null 'output' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
