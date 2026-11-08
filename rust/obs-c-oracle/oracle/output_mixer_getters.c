/* Real-layout original C getters with callback-only diagnostic probes. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_first_mixer oracle_obs_output_first_mixer
#define obs_output_get_mixer oracle_obs_output_get_mixer
#define obs_output_get_mixers oracle_obs_output_get_mixers
#include "obs-output-mixer-getters.c.h"
#undef obs_output_get_mixers
#undef obs_output_get_mixer
#undef obs_output_first_mixer
#include "output_mixer_getters_fields.inc"

struct output_mixer_fixture {
	struct obs_output output;
	struct obs_output before;
};
void *oracle_output_mixer_new(size_t mask, uint32_t flags);
void *oracle_output_mixer_new(size_t mask, uint32_t flags)
{
	struct output_mixer_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.mixer_mask = mask;
		fixture->output.info.flags = flags;
		fixture->output.owns_info_id = true;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}
bool oracle_output_mixer_unchanged(const void *handle);
bool oracle_output_mixer_unchanged(const void *handle)
{
	const struct output_mixer_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}
void oracle_output_mixer_free(void *handle);
void oracle_output_mixer_free(void *handle)
{
	free(handle);
}
bool oracle_output_mixer_addresses(const obs_output_t *output);
bool oracle_output_mixer_addresses(const obs_output_t *output)
{
	return obs_output_mixer_getters_fields(output).mask == &output->mixer_mask;
}
void oracle_output_mixer_layout(size_t *layout);
void oracle_output_mixer_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_mixer_getters_fields);
	layout[1] = _Alignof(struct obs_output_mixer_getters_fields);
	layout[2] = offsetof(struct obs_output_mixer_getters_fields, mask);
	layout[3] = sizeof(((struct obs_output *)0)->mixer_mask);
	layout[4] = _Alignof(size_t);
}

struct output_mixer_getters_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_mixer_getters_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_mixer_getters_capture(int level, const char *format, va_list args, void *param)
{
	struct output_mixer_getters_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef size_t (*output_mixer_getters_getter)(const obs_output_t *);
typedef void (*output_mixer_getters_get_log)(log_handler_t *, void **);
typedef void (*output_mixer_getters_set_log)(log_handler_t, void *);
bool oracle_output_mixer_getters_logs(output_mixer_getters_getter video, output_mixer_getters_getter audio,
				      output_mixer_getters_get_log get_log, output_mixer_getters_set_log set_log);
bool oracle_output_mixer_getters_logs(output_mixer_getters_getter video, output_mixer_getters_getter audio,
				      output_mixer_getters_get_log get_log, output_mixer_getters_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	output_mixer_getters_getter rust[] = {video, audio};
	output_mixer_getters_getter original[] = {oracle_obs_output_get_mixer, oracle_obs_output_get_mixers};
	const char *expected[] = {"obs_output_get_mixer: Null 'output' parameter",
				  "obs_output_get_mixers: Null 'output' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		struct output_mixer_getters_log r = {0}, c = {0};
		set_log(output_mixer_getters_capture, &r);
		size_t rust_value = rust[i](NULL);
		set_log(output_mixer_getters_capture, &c);
		size_t c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
