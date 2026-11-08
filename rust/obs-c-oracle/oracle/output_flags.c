/* Original C query, real-layout adapter and callback-only diagnostic probe. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_flags oracle_obs_output_get_flags
#include "obs-output-flags.c.h"
#undef obs_output_get_flags
#include "output_flags_fields.inc"

struct output_flags_fixture {
	struct obs_output output;
	struct obs_output before;
};
void *oracle_output_flags_new(uint32_t flags, size_t mask);
void *oracle_output_flags_new(uint32_t flags, size_t mask)
{
	struct output_flags_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.mixer_mask = mask;
		fixture->output.info.flags = flags;
		fixture->output.owns_info_id = true;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}
bool oracle_output_flags_unchanged(const void *handle);
bool oracle_output_flags_unchanged(const void *handle)
{
	const struct output_flags_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}
void oracle_output_flags_free(void *handle);
void oracle_output_flags_free(void *handle)
{
	free(handle);
}
bool oracle_output_flags_addresses(const obs_output_t *output);
bool oracle_output_flags_addresses(const obs_output_t *output)
{
	return obs_output_flags_fields(output).flags == &output->info.flags;
}
void oracle_output_flags_layout(size_t *layout);
void oracle_output_flags_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_flags_fields);
	layout[1] = _Alignof(struct obs_output_flags_fields);
	layout[2] = offsetof(struct obs_output_flags_fields, flags);
	layout[3] = sizeof(((struct obs_output *)0)->info.flags);
	layout[4] = _Alignof(uint32_t);
}

struct output_flags_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_flags_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_flags_capture(int level, const char *format, va_list args, void *param)
{
	struct output_flags_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef uint32_t (*output_flags_getter)(const obs_output_t *);
typedef void (*output_flags_get_log)(log_handler_t *, void **);
typedef void (*output_flags_set_log)(log_handler_t, void *);
bool oracle_output_flags_logs(output_flags_getter getter, output_flags_get_log get_log, output_flags_set_log set_log);
bool oracle_output_flags_logs(output_flags_getter getter, output_flags_get_log get_log, output_flags_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_flags_log r = {0}, c = {0};
	set_log(output_flags_capture, &r);
	uint32_t rust_value = getter(NULL);
	set_log(output_flags_capture, &c);
	uint32_t c_value = oracle_obs_output_get_flags(NULL);
	set_log(old_handler, old_param);
	return !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG && strcmp(r.message, "obs_output_get_flags: Null 'output' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
