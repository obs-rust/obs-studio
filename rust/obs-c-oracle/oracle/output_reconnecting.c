/* Original extracted C over the real output layout. No behavior exclusions. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_reconnecting oracle_obs_output_reconnecting
#include "obs-output-reconnecting.c.h"
#undef obs_output_reconnecting
#include "output_reconnecting_fields.inc"

extern bool obs_output_reconnecting(const obs_output_t *output);

void oracle_output_reconnecting_layout(size_t *layout);
void oracle_output_reconnecting_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_reconnecting_fields);
	layout[1] = _Alignof(struct obs_output_reconnecting_fields);
	layout[2] = offsetof(struct obs_output_reconnecting_fields, reconnecting);
	layout[3] = sizeof(bool);
	layout[4] = _Alignof(bool);
}

struct output_reconnecting_fixture {
	struct obs_output output;
	struct obs_output before;
};
void *oracle_output_reconnecting_new(bool flag, bool active_flag, bool paused_flag);
void *oracle_output_reconnecting_new(bool flag, bool active_flag, bool paused_flag)
{
	struct output_reconnecting_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		os_atomic_set_bool(&fixture->output.reconnecting, flag);
		os_atomic_set_bool(&fixture->output.active, active_flag);
		os_atomic_set_bool(&fixture->output.paused, paused_flag);
		fixture->output.reconnect_retry_cur_msec = 29;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_output_reconnecting_unchanged(void *output);
bool oracle_output_reconnecting_unchanged(void *output)
{
	struct output_reconnecting_fixture *fixture = output;
	return memcmp(&fixture->output, &fixture->before, sizeof(fixture->output)) == 0;
}
void oracle_output_reconnecting_free(void *output);
void oracle_output_reconnecting_free(void *output)
{
	free(output);
}

struct output_reconnecting_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_reconnecting_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_reconnecting_capture(int level, const char *format, va_list args, void *param)
{
	struct output_reconnecting_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_output_reconnecting_null_log(bool rust);
bool oracle_output_reconnecting_null_log(bool rust)
{
	struct output_reconnecting_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(output_reconnecting_capture, &log);
	bool flag = rust ? obs_output_reconnecting(NULL) : oracle_obs_output_reconnecting(NULL);
	base_set_log_handler(old_handler, old_param);
	return !flag && log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_output_reconnecting: Null 'output' parameter") == 0;
}
