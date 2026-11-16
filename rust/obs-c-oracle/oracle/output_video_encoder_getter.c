/* Original C and discovered adapter over the real private output layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_slots_video_slot oracle_obs_output_slots_video_slot
#define obs_output_get_video_encoder2 oracle_obs_output_get_video_encoder2
#include "obs-output-video-encoder-getter.c.h"
#undef obs_output_get_video_encoder2
#undef obs_output_slots_video_slot
#include "output_video_encoder_getter_fields.inc"

struct output_video_encoder_fixture {
	struct obs_output output;
	struct obs_output before;
	struct obs_encoder encoders[MAX_OUTPUT_VIDEO_ENCODERS];
};

void *oracle_output_video_encoder_new(uint32_t occupied, uint32_t flags);
void *oracle_output_video_encoder_new(uint32_t occupied, uint32_t flags)
{
	struct output_video_encoder_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		for (size_t idx = 0; idx < MAX_OUTPUT_VIDEO_ENCODERS; idx++)
			fixture->output.video_encoders[idx] = (occupied & (1u << idx)) ? &fixture->encoders[idx] : NULL;
		fixture->output.info.flags = flags;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->output));
	}
	return fixture;
}

void *oracle_output_video_encoder_expected(const void *handle, size_t idx);
void *oracle_output_video_encoder_expected(const void *handle, size_t idx)
{
	const struct output_video_encoder_fixture *fixture = handle;
	return idx < MAX_OUTPUT_VIDEO_ENCODERS ? fixture->before.video_encoders[idx] : NULL;
}

bool oracle_output_video_encoder_unchanged(const void *handle);
bool oracle_output_video_encoder_unchanged(const void *handle)
{
	const struct output_video_encoder_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->output, sizeof(fixture->output)) == 0;
}

void oracle_output_video_encoder_free(void *handle);
void oracle_output_video_encoder_free(void *handle)
{
	free(handle);
}

struct output_video_encoder_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_video_encoder_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_video_encoder_capture(int level, const char *format, va_list args, void *param)
{
	struct output_video_encoder_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Symbol renaming changes __func__ only in the oracle. Strip just that
	 * oracle-only prefix so diagnostics retain the public function name. */
	if (strncmp(log->message, "oracle_", 7) == 0)
		memmove(log->message, log->message + 7, strlen(log->message + 7) + 1);
}

typedef void *(*output_video_encoder_getter)(const obs_output_t *, size_t);
typedef void (*output_video_encoder_get_log)(log_handler_t *, void **);
typedef void (*output_video_encoder_set_log)(log_handler_t, void *);
bool oracle_output_video_encoder_logs(output_video_encoder_getter getter, output_video_encoder_get_log get_log,
				      output_video_encoder_set_log set_log, size_t idx);
bool oracle_output_video_encoder_logs(output_video_encoder_getter getter, output_video_encoder_get_log get_log,
				      output_video_encoder_set_log set_log, size_t idx)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_video_encoder_log r = {0}, c = {0};
	set_log(output_video_encoder_capture, &r);
	void *rust_value = getter(NULL, idx);
	set_log(output_video_encoder_capture, &c);
	void *c_value = oracle_obs_output_get_video_encoder2(NULL, idx);
	set_log(old_handler, old_param);
	return !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
	       c.level == LOG_DEBUG &&
	       strcmp(r.message, "obs_output_get_video_encoder2: Null 'output' parameter") == 0 &&
	       strcmp(r.message, c.message) == 0;
}
