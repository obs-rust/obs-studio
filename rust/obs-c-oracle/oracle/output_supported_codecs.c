/* Original extracted getters and discovered field adapter over real C layouts. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_supported_video_codecs oracle_obs_output_get_supported_video_codecs
#define obs_output_get_supported_audio_codecs oracle_obs_output_get_supported_audio_codecs
#include "obs-output-supported-codecs.c.h"
#undef obs_output_get_supported_audio_codecs
#undef obs_output_get_supported_video_codecs
#include "output_supported_codecs_fields.inc"

struct output_codecs_fixture {
	struct obs_output output;
	struct obs_output before;
};

void *oracle_output_codecs_new(const char *video, const char *audio, uint32_t flags);
void *oracle_output_codecs_new(const char *video, const char *audio, uint32_t flags)
{
	struct output_codecs_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.info.encoded_video_codecs = video;
		fixture->output.info.encoded_audio_codecs = audio;
		fixture->output.info.flags = flags;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}

bool oracle_output_codecs_unchanged(const void *handle);
bool oracle_output_codecs_unchanged(const void *handle)
{
	const struct output_codecs_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}

void oracle_output_codecs_free(void *handle);
void oracle_output_codecs_free(void *handle)
{
	free(handle);
}

bool oracle_output_codecs_addresses(const obs_output_t *output);
bool oracle_output_codecs_addresses(const obs_output_t *output)
{
	struct obs_output_supported_codecs_fields fields = obs_output_supported_codecs_fields(output);
	return fields.video == &output->info.encoded_video_codecs && fields.audio == &output->info.encoded_audio_codecs;
}

void oracle_output_codecs_layout(size_t *layout);
void oracle_output_codecs_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_supported_codecs_fields);
	layout[1] = _Alignof(struct obs_output_supported_codecs_fields);
	layout[2] = offsetof(struct obs_output_supported_codecs_fields, video);
	layout[3] = offsetof(struct obs_output_supported_codecs_fields, audio);
	layout[4] = sizeof(((struct obs_output *)0)->info.encoded_video_codecs);
	layout[5] = _Alignof(const char *);
}

struct output_codecs_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_codecs_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_codecs_capture(int level, const char *format, va_list args, void *param)
{
	struct output_codecs_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef const char *(*output_codecs_getter)(const obs_output_t *);
typedef void (*output_codecs_get_log)(log_handler_t *, void **);
typedef void (*output_codecs_set_log)(log_handler_t, void *);
bool oracle_output_codecs_logs(output_codecs_getter video, output_codecs_getter audio, output_codecs_get_log get_log,
			       output_codecs_set_log set_log);
bool oracle_output_codecs_logs(output_codecs_getter video, output_codecs_getter audio, output_codecs_get_log get_log,
			       output_codecs_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	output_codecs_getter rust[] = {video, audio};
	output_codecs_getter original[] = {oracle_obs_output_get_supported_video_codecs,
					   oracle_obs_output_get_supported_audio_codecs};
	const char *expected[] = {"obs_output_get_supported_video_codecs: Null 'output' parameter",
				  "obs_output_get_supported_audio_codecs: Null 'output' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		struct output_codecs_log r = {0}, c = {0};
		set_log(output_codecs_capture, &r);
		const char *rust_value = rust[i](NULL);
		set_log(output_codecs_capture, &c);
		const char *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
