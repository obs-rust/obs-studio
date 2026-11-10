/* Exact extracted setter over the real C output layout; no exclusions. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_set_reconnect_settings oracle_obs_output_set_reconnect_settings
#include "obs-output-reconnect-settings.c.h"
#undef obs_output_set_reconnect_settings
#include "output_reconnect_settings_fields.inc"

extern void obs_output_set_reconnect_settings(obs_output_t *output, int retry_count, int retry_sec);

void oracle_output_reconnect_settings_layout(size_t *layout);
void oracle_output_reconnect_settings_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_reconnect_settings_fields);
	layout[1] = _Alignof(struct obs_output_reconnect_settings_fields);
	layout[2] = offsetof(struct obs_output_reconnect_settings_fields, retry_max);
	layout[3] = offsetof(struct obs_output_reconnect_settings_fields, retry_sec);
}

struct output_reconnect_settings_fixture {
	struct obs_output output;
	struct obs_output expected;
};

void *oracle_output_reconnect_settings_new(void);
void *oracle_output_reconnect_settings_new(void)
{
	struct output_reconnect_settings_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.reconnect_retries = 17;
		fixture->output.reconnect_retry_cur_msec = 1234;
		fixture->output.reconnect_retry_exp = 1.5f;
		fixture->output.reconnecting = true;
		memcpy(&fixture->expected, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}

bool oracle_output_reconnect_settings_check(void *output, int retry_count, int retry_sec);
bool oracle_output_reconnect_settings_check(void *output, int retry_count, int retry_sec)
{
	struct output_reconnect_settings_fixture *fixture = output;
	fixture->expected.reconnect_retry_max = retry_count;
	fixture->expected.reconnect_retry_sec = retry_sec;
	return memcmp(&fixture->output, &fixture->expected, sizeof(fixture->output)) == 0;
}

void oracle_output_reconnect_settings_free(void *output);
void oracle_output_reconnect_settings_free(void *output)
{
	free(output);
}

struct output_reconnect_settings_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_reconnect_settings_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_reconnect_settings_capture(int level, const char *format, va_list args, void *param)
{
	struct output_reconnect_settings_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_output_reconnect_settings_null_log(bool rust);
bool oracle_output_reconnect_settings_null_log(bool rust)
{
	struct output_reconnect_settings_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(output_reconnect_settings_capture, &log);
	if (rust)
		obs_output_set_reconnect_settings(NULL, -1, 0);
	else
		oracle_obs_output_set_reconnect_settings(NULL, -1, 0);
	base_set_log_handler(old_handler, old_param);
	return log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_output_set_reconnect_settings: Null 'output' parameter") == 0;
}
