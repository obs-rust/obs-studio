/* Original extracted C query over the real private output layout. No exclusions. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_active oracle_obs_output_active
#include "obs-output-active.c.h"
#undef obs_output_active
#include "output_active_fields.inc"

extern bool obs_output_active(const obs_output_t *output);

void oracle_output_active_layout(size_t *layout);
void oracle_output_active_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_active_fields);
	layout[1] = _Alignof(struct obs_output_active_fields);
	layout[2] = offsetof(struct obs_output_active_fields, active);
	layout[3] = offsetof(struct obs_output_active_fields, reconnecting);
	layout[4] = sizeof(bool);
	layout[5] = _Alignof(bool);
}

struct output_active_fixture {
	struct obs_output output;
	struct obs_output before;
};
void *oracle_output_active_new(bool active_flag, bool reconnecting_flag, bool paused_flag);
void *oracle_output_active_new(bool active_flag, bool reconnecting_flag, bool paused_flag)
{
	struct output_active_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		os_atomic_set_bool(&fixture->output.active, active_flag);
		os_atomic_set_bool(&fixture->output.reconnecting, reconnecting_flag);
		os_atomic_set_bool(&fixture->output.paused, paused_flag);
		fixture->output.reconnect_retry_cur_msec = 29;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_output_active_unchanged(void *output);
bool oracle_output_active_unchanged(void *output)
{
	struct output_active_fixture *fixture = output;
	return memcmp(&fixture->output, &fixture->before, sizeof(fixture->output)) == 0;
}
void oracle_output_active_free(void *output);
void oracle_output_active_free(void *output)
{
	free(output);
}

struct output_active_log {
	unsigned count;
};
#if defined(__GNUC__) || defined(__clang__)
static void output_active_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_active_capture(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	struct output_active_log *log = param;
	log->count++;
}
bool oracle_output_active_null_silent(bool rust);
bool oracle_output_active_null_silent(bool rust)
{
	struct output_active_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(output_active_capture, &log);
	bool flag = rust ? obs_output_active(NULL) : oracle_obs_output_active(NULL);
	base_set_log_handler(old_handler, old_param);
	return !flag && log.count == 0;
}
