/* Original extracted C over the real output layout. No behavior exclusions. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_total_frames oracle_obs_output_get_total_frames
#include "obs-output-total-frames.c.h"
#undef obs_output_get_total_frames
#include "output_total_frames_fields.inc"

extern int obs_output_get_total_frames(const obs_output_t *output);

void oracle_output_total_frames_layout(size_t *layout);
void oracle_output_total_frames_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_total_frames_fields);
	layout[1] = _Alignof(struct obs_output_total_frames_fields);
	layout[2] = offsetof(struct obs_output_total_frames_fields, frames);
	layout[3] = sizeof(long);
	layout[4] = _Alignof(long);
}

struct output_total_frames_fixture {
	struct obs_output output;
	struct obs_output before;
};
void *oracle_output_total_frames_new(int64_t count);
void *oracle_output_total_frames_new(int64_t count)
{
	struct output_total_frames_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		os_atomic_set_long(&fixture->output.total_frames, (long)count);
		fixture->output.starting_lagged_count = 17;
		fixture->output.starting_drawn_count = 29;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->before));
	}
	return fixture;
}
bool oracle_output_total_frames_unchanged(void *output);
bool oracle_output_total_frames_unchanged(void *output)
{
	struct output_total_frames_fixture *fixture = output;
	return memcmp(&fixture->output, &fixture->before, sizeof(fixture->output)) == 0;
}
void oracle_output_total_frames_free(void *output);
void oracle_output_total_frames_free(void *output)
{
	free(output);
}

struct output_total_frames_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_total_frames_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_total_frames_capture(int level, const char *format, va_list args, void *param)
{
	struct output_total_frames_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_output_total_frames_null_log(bool rust);
bool oracle_output_total_frames_null_log(bool rust)
{
	struct output_total_frames_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(output_total_frames_capture, &log);
	int frames = rust ? obs_output_get_total_frames(NULL) : oracle_obs_output_get_total_frames(NULL);
	base_set_log_handler(old_handler, old_param);
	return frames == 0 && log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_output_get_total_frames: Null 'output' parameter") == 0;
}
