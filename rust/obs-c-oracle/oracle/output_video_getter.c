/* Original C output video getter over real-header output/encoder fixtures.
 * Its dependencies stay original C: obs_output_get_video_encoder() and
 * obs_encoder_video() are the renamed originals of their own oracles
 * (output_encoder_getters.c, encoder_media_getters.c), and flag_encoded() is
 * obs-output.c.h's file-local helper, unchanged. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"

video_t *oracle_obs_output_video(const obs_output_t *output);
obs_encoder_t *oracle_obs_output_get_video_encoder(const obs_output_t *output);
video_t *oracle_obs_encoder_video(const obs_encoder_t *encoder);
#define obs_output_video oracle_obs_output_video
#define obs_output_get_video_encoder oracle_obs_output_get_video_encoder
#define obs_encoder_video oracle_obs_encoder_video
#include "output-video-getter.inc"
#undef obs_encoder_video
#undef obs_output_get_video_encoder
#undef obs_output_video

/* obs-output.c.h's static flag_encoded(), unchanged. */
static inline bool flag_encoded(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_ENCODED) != 0;
}

/* The C characterization's fixture, widened to every video slot. */
struct output_video_fixture {
	struct obs_output output;
	struct obs_encoder encoders[MAX_OUTPUT_VIDEO_ENCODERS];
	struct obs_weak_object controls[MAX_OUTPUT_VIDEO_ENCODERS + 1];
	uint64_t raw_token;
	uint64_t media_tokens[MAX_OUTPUT_VIDEO_ENCODERS];
	uint64_t override_tokens[MAX_OUTPUT_VIDEO_ENCODERS];
};

/* Bit i of each mask describes slot i: occupied, a video (not audio) encoder,
 * media set, FPS override set. poison fills every slot with an unreadable
 * pointer (raw outputs only). */
void *oracle_output_video_new(uint32_t flags, bool raw, uint32_t occupied, uint32_t video, uint32_t media,
			      uint32_t override, bool poison);
void *oracle_output_video_new(uint32_t flags, bool raw, uint32_t occupied, uint32_t video, uint32_t media,
			      uint32_t override, bool poison)
{
	struct output_video_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->output.context.control = &f->controls[MAX_OUTPUT_VIDEO_ENCODERS];
	f->output.info.flags = flags;
	f->output.video = raw ? (video_t *)&f->raw_token : NULL;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		struct obs_encoder *encoder = &f->encoders[i];
		encoder->context.control = &f->controls[i];
		encoder->context.name = "video-%s";
		encoder->info.type = (video & (1u << i)) ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
		encoder->media = (media & (1u << i)) ? (video_t *)&f->media_tokens[i] : NULL;
		encoder->fps_override = (override & (1u << i)) ? (video_t *)&f->override_tokens[i] : NULL;
		f->output.video_encoders[i] = poison                   ? (obs_encoder_t *)(uintptr_t)1
					      : (occupied & (1u << i)) ? encoder
								       : NULL;
	}
	for (size_t i = 0; i <= MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		f->controls[i].ref.refs = 7;
		f->controls[i].ref.weak_refs = 11;
	}
	return f;
}

void oracle_output_video_free(void *handle);
void oracle_output_video_free(void *handle)
{
	free(handle);
}

struct output_video_log {
	unsigned count;
	int level;
	char message[256];
};

#if defined(__GNUC__) || defined(__clang__)
static void output_video_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_video_capture(int level, const char *format, va_list args, void *param)
{
	struct output_video_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

/* What a pointer is: 0 NULL, 1 the raw video, 2 + i slot i's media,
 * 2 + MAX + i slot i's override, UINT64_MAX anything else. */
static uint64_t output_video_identity(const struct output_video_fixture *f, const video_t *video)
{
	if (!video)
		return 0;
	if (video == (const video_t *)&f->raw_token)
		return 1;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (video == (const video_t *)&f->media_tokens[i])
			return 2 + i;
		if (video == (const video_t *)&f->override_tokens[i])
			return 2 + MAX_OUTPUT_VIDEO_ENCODERS + i;
	}
	return UINT64_MAX;
}

typedef video_t *(*output_video_getter)(const obs_output_t *);
typedef void (*output_video_get_log)(log_handler_t *, void **);
typedef void (*output_video_set_log)(log_handler_t, void *);

/* report: identity, log count, log level, fixture unchanged (including the
 * strong/weak reference counters). message receives the last diagnostic. */
void oracle_output_video_run(void *handle, output_video_getter getter, output_video_get_log get_log,
			     output_video_set_log set_log, uint64_t *report, char *message, size_t message_size);
void oracle_output_video_run(void *handle, output_video_getter getter, output_video_get_log get_log,
			     output_video_set_log set_log, uint64_t *report, char *message, size_t message_size)
{
	struct output_video_fixture *f = handle;
	struct output_video_fixture *before = NULL;
	if (f) {
		before = malloc(sizeof(*before));
		if (!before)
			abort();
		memcpy(before, f, sizeof(*f));
	}
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_video_log log = {0};
	set_log(output_video_capture, &log);
	video_t *video = getter(f ? &f->output : NULL);
	set_log(old_handler, old_param);
	report[0] = f ? output_video_identity(f, video) : (video ? UINT64_MAX : 0);
	report[1] = log.count;
	report[2] = (uint64_t)(int64_t)log.level;
	report[3] = !f || memcmp(f, before, sizeof(*f)) == 0;
	snprintf(message, message_size, "%s", log.message);
	free(before);
}
