/* Original C query and production field seam over the real private header. */
#define obs_output_can_pause oracle_output_can_pause
#include "libobs/obs-output-can-pause.c.h"
#undef obs_output_can_pause
#include "output-can-pause-fields.inc"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

extern bool obs_output_can_pause(const obs_output_t *output);

struct output_pause_fixture {
	struct obs_output output;
	unsigned char before[sizeof(struct obs_output)];
};

void *oracle_output_pause_new(uint32_t flags);
void *oracle_output_pause_new(uint32_t flags)
{
	struct output_pause_fixture *f = calloc(1, sizeof(*f));
	if (f) {
		f->output.info.flags = flags;
		f->output.owns_info_id = true;
		f->output.mixer_mask = 37;
		memcpy(f->before, &f->output, sizeof(f->output));
	}
	return f;
}

bool oracle_output_pause_unchanged(const void *p);
bool oracle_output_pause_unchanged(const void *p)
{
	const struct output_pause_fixture *f = p;
	return memcmp(&f->output, f->before, sizeof(f->output)) == 0;
}

void oracle_output_pause_free(void *p);
void oracle_output_pause_free(void *p)
{
	free(p);
}

void oracle_output_pause_layout(size_t *values);
void oracle_output_pause_layout(size_t *values)
{
	values[0] = sizeof(uint32_t);
	values[1] = _Alignof(uint32_t);
	values[2] = OBS_OUTPUT_CAN_PAUSE;
}

bool oracle_output_pause_field_address(const obs_output_t *output);
bool oracle_output_pause_field_address(const obs_output_t *output)
{
	return obs_output_can_pause_flags(output) == &output->info.flags;
}

struct output_pause_log {
	unsigned calls;
	int level;
	char message[128];
};

#if defined(__GNUC__) || defined(__clang__)
static void output_pause_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_pause_capture(int level, const char *format, va_list args, void *param)
{
	struct output_pause_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_output_pause_logs(bool rust);
bool oracle_output_pause_logs(bool rust)
{
	struct output_pause_log log = {0};
	struct obs_output output = {0};
	log_handler_t old;
	void *param;
	base_get_log_handler(&old, &param);
	base_set_log_handler(output_pause_capture, &log);
	bool result = rust ? obs_output_can_pause(NULL) : oracle_output_can_pause(NULL);
	unsigned after_null = log.calls;
	bool present = rust ? obs_output_can_pause(&output) : oracle_output_can_pause(&output);
	base_set_log_handler(old, param);
	return !result && !present && after_null == 1 && log.calls == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_output_can_pause: Null 'output' parameter") == 0;
}
