/* Original setter and the real writable-field adapter. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_set_last_error oracle_obs_output_set_last_error
#include "obs-output-set-last-error.c.h"
#undef obs_output_set_last_error
#include "output_last_error_fields.inc"

struct output_last_error_fixture {
	struct obs_output output;
	struct obs_output before;
};

void *oracle_output_last_error_new(void);
void *oracle_output_last_error_new(void)
{
	struct output_last_error_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.context.name = "independent-output";
		fixture->output.mixer_mask = UINT32_C(0x80000001);
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->before));
	}
	return fixture;
}

const char *oracle_output_last_error_read(obs_output_t *output);
const char *oracle_output_last_error_read(obs_output_t *output)
{
	return output->last_error_message;
}

bool oracle_output_last_error_unchanged(obs_output_t *output);
bool oracle_output_last_error_unchanged(obs_output_t *output)
{
	struct output_last_error_fixture *fixture = (void *)output;
	fixture->before.last_error_message = output->last_error_message;
	return memcmp(output, &fixture->before, sizeof(*output)) == 0;
}

void oracle_output_last_error_free(obs_output_t *output);
void oracle_output_last_error_free(obs_output_t *output)
{
	bfree(output->last_error_message);
	free(output);
}

typedef void (*output_last_error_setter)(obs_output_t *, const char *);
struct output_last_error_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_last_error_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_last_error_capture(int level, const char *format, va_list args, void *param)
{
	struct output_last_error_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_output_last_error_null_log(output_last_error_setter setter);
bool oracle_output_last_error_null_log(output_last_error_setter setter)
{
	struct output_last_error_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(output_last_error_capture, &log);
	setter(NULL, NULL);
	bool ok = log.count == 1 && log.level == LOG_DEBUG &&
		  strcmp(log.message, "obs_output_set_last_error: Null 'output' parameter") == 0;
	base_set_log_handler(old_handler, old_param);
	return ok;
}
