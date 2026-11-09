/* Original C getter and production field adapter over the real private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_settings oracle_obs_output_get_settings
#include "obs-output-settings.c.h"
#undef obs_output_get_settings
#include "output_settings_fields.inc"

void *oracle_output_settings_new(obs_data_t *settings, uint32_t flags);
void *oracle_output_settings_new(obs_data_t *settings, uint32_t flags)
{
	struct obs_output *output = calloc(1, sizeof(*output));
	if (output) {
		output->context.settings = settings;
		output->info.flags = flags;
	}
	return output;
}

void oracle_output_settings_free(void *output);
void oracle_output_settings_free(void *output)
{
	free(output);
}

bool oracle_output_settings_addresses(const obs_output_t *output);
bool oracle_output_settings_addresses(const obs_output_t *output)
{
	return obs_output_settings_fields(output) == &output->context.settings;
}

struct output_settings_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_settings_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_settings_capture(int level, const char *format, va_list args, void *param)
{
	struct output_settings_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef void *(*output_settings_getter)(const obs_output_t *);
typedef void (*output_settings_get_log)(log_handler_t *, void **);
typedef void (*output_settings_set_log)(log_handler_t, void *);
bool oracle_output_settings_logs(output_settings_getter getter, output_settings_get_log get_log,
				 output_settings_set_log set_log);
bool oracle_output_settings_logs(output_settings_getter getter, output_settings_get_log get_log,
				 output_settings_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_settings_log r = {0}, c = {0};
	set_log(output_settings_capture, &r);
	void *rust_value = getter(NULL);
	set_log(output_settings_capture, &c);
	void *c_value = oracle_obs_output_get_settings(NULL);
	set_log(old_handler, old_param);
	return !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG && strcmp(r.message, "obs_output_get_settings: Null 'output' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
