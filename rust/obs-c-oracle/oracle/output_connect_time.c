/* Original extracted C over the real output layout. No behavior exclusions. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_connect_time_ms oracle_obs_output_get_connect_time_ms
#include "obs-output-connect-time.c.h"
#undef obs_output_get_connect_time_ms
#include "output_connect_time_fields.inc"

extern int obs_output_get_connect_time_ms(obs_output_t *output);

void oracle_output_connect_time_layout(size_t *layout);
void oracle_output_connect_time_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_connect_time_fields);
	layout[1] = _Alignof(struct obs_output_connect_time_fields);
	layout[2] = offsetof(struct obs_output_connect_time_fields, data);
	layout[3] = offsetof(struct obs_output_connect_time_fields, callback);
}
struct output_connect_time_fixture {
	struct obs_output output;
	struct obs_output before;
	int result;
	unsigned calls;
	bool identity;
};
static int output_connect_time_callback(void *data)
{
	struct output_connect_time_fixture *fixture = data;
	fixture->calls++;
	fixture->identity = fixture->output.context.data == fixture;
	return fixture->result;
}
/* A stored NULL must reach the callback unchanged. */
static int output_connect_time_null_data(void *data)
{
	return data == NULL ? -73 : 91;
}
void *oracle_output_connect_time_new(bool callback, int result, bool null_data);
void *oracle_output_connect_time_new(bool callback, int result, bool null_data)
{
	struct output_connect_time_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->result = result;
		fixture->output.context.data = null_data ? NULL : fixture;
		fixture->output.info.get_connect_time_ms =
			callback ? (null_data ? output_connect_time_null_data : output_connect_time_callback) : NULL;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_output_connect_time_unchanged(void *output, unsigned calls);
bool oracle_output_connect_time_unchanged(void *output, unsigned calls)
{
	struct output_connect_time_fixture *fixture = output;
	return fixture->calls == calls && (!calls || fixture->identity) &&
	       memcmp(&fixture->output, &fixture->before, sizeof(fixture->output)) == 0;
}
void oracle_output_connect_time_free(void *output);
void oracle_output_connect_time_free(void *output)
{
	free(output);
}

struct output_connect_time_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_connect_time_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_connect_time_capture(int level, const char *format, va_list args, void *param)
{
	struct output_connect_time_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_output_connect_time_null_log(bool rust);
bool oracle_output_connect_time_null_log(bool rust)
{
	struct output_connect_time_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(output_connect_time_capture, &log);
	int result = rust ? obs_output_get_connect_time_ms(NULL) : oracle_obs_output_get_connect_time_ms(NULL);
	base_set_log_handler(old_handler, old_param);
	return result == -1 && log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_output_get_connect_time_ms: Null 'output' parameter") == 0;
}

/* Call only in the logger-isolated test process. */
bool oracle_output_connect_time_valid_silent(bool rust, void *output);
bool oracle_output_connect_time_valid_silent(bool rust, void *output)
{
	struct output_connect_time_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(output_connect_time_capture, &log);
	if (rust)
		obs_output_get_connect_time_ms(output);
	else
		oracle_obs_output_get_connect_time_ms(output);
	base_set_log_handler(old_handler, old_param);
	return log.count == 0;
}
