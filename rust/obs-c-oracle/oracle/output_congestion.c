/* Original extracted query and original C clamp over the real output layout. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
extern float oracle_obs_output_congestion_clamp(float congestion);
#define obs_output_congestion_clamp oracle_obs_output_congestion_clamp
#define obs_output_get_congestion oracle_obs_output_get_congestion
#include "obs-output-congestion.c.h"
#undef obs_output_get_congestion
#undef obs_output_congestion_clamp
#include "output_congestion_fields.inc"

extern float obs_output_get_congestion(obs_output_t *output);

void oracle_output_congestion_layout(size_t *layout);
void oracle_output_congestion_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_congestion_fields);
	layout[1] = _Alignof(struct obs_output_congestion_fields);
	layout[2] = offsetof(struct obs_output_congestion_fields, callback);
	layout[3] = offsetof(struct obs_output_congestion_fields, data);
}

struct output_congestion_fixture {
	struct obs_output output;
	struct obs_output before;
	uint32_t bits;
	unsigned calls;
};
static float output_congestion_callback(void *data)
{
	struct output_congestion_fixture *fixture = data;
	float result;
	memcpy(&result, &fixture->bits, sizeof(result));
	fixture->calls++;
	return result;
}
void *oracle_output_congestion_new(uint32_t bits, bool callback);
void *oracle_output_congestion_new(uint32_t bits, bool callback)
{
	struct output_congestion_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->bits = bits;
		fixture->output.context.data = fixture;
		fixture->output.info.get_congestion = callback ? output_congestion_callback : NULL;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_output_congestion_unchanged(void *output, unsigned calls);
bool oracle_output_congestion_unchanged(void *output, unsigned calls)
{
	struct output_congestion_fixture *fixture = output;
	return fixture->calls == calls && memcmp(&fixture->output, &fixture->before, sizeof(fixture->output)) == 0;
}
void oracle_output_congestion_free(void *output);
void oracle_output_congestion_free(void *output)
{
	free(output);
}

struct output_congestion_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_congestion_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_congestion_capture(int level, const char *format, va_list args, void *param)
{
	struct output_congestion_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_output_congestion_null_log(bool rust);
bool oracle_output_congestion_null_log(bool rust)
{
	struct output_congestion_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(output_congestion_capture, &log);
	float result = rust ? obs_output_get_congestion(NULL) : oracle_obs_output_get_congestion(NULL);
	base_set_log_handler(old_handler, old_param);
	return result == 0 && log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_output_get_congestion: Null 'output' parameter") == 0;
}
