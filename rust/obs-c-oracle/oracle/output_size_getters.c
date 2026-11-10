/* Compile the original output getters, their original policy and encoder getters. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_width2 oracle_obs_output_get_width2
#define obs_output_get_width oracle_obs_output_get_width
#define obs_output_get_height2 oracle_obs_output_get_height2
#define obs_output_get_height oracle_obs_output_get_height
#define obs_encoder_size_video_route oracle_obs_encoder_size_video_route
#define obs_output_size_route oracle_obs_output_size_route
#define obs_encoder_get_width output_size_encoder_width
#define obs_encoder_get_height output_size_encoder_height
#include "obs-output-size-getters.c.h"
#undef obs_output_get_width2
#undef obs_output_get_width
#undef obs_output_get_height2
#undef obs_output_get_height
#undef obs_output_size_route
#define obs_encoder_get_name output_size_encoder_name
static const char *output_size_encoder_name(const obs_encoder_t *encoder)
{
	return encoder->context.name;
}
#include "obs-encoder-dimensions.c.h"
#undef obs_encoder_get_name
#undef obs_encoder_size_video_route
#undef obs_encoder_get_width
#undef obs_encoder_get_height
#include "output_size_getters_fields.inc"

static inline bool flag_encoded(const struct obs_output *output)
{
	return (output->info.flags & OBS_OUTPUT_ENCODED) != 0;
}
static inline bool log_flag_video(const struct obs_output *output, const char *func_name)
{
	bool ret = (output->info.flags & OBS_OUTPUT_VIDEO) != 0;
	if (!ret)
		blog(LOG_WARNING, "Output '%s': Tried to use %s on a non-video output", output->context.name, func_name);
	return ret;
}
struct output_size_fixture {
	struct obs_output output;
	struct obs_output before;
	struct obs_encoder encoder;
	struct obs_encoder encoder_before;
};
void *oracle_output_size_new(uint32_t flags, video_t *video, bool media, uint32_t width, uint32_t height, size_t slot,
			     unsigned present);
void *oracle_output_size_new(uint32_t flags, video_t *video, bool media, uint32_t width, uint32_t height, size_t slot,
			     unsigned present)
{
	struct output_size_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->output.context.name = "size-%s";
	f->output.info.flags = flags;
	f->output.video = media ? video : NULL;
	f->output.scaled_width = width;
	f->output.scaled_height = height;
	f->encoder.info.type = OBS_ENCODER_VIDEO;
	f->encoder.media = present == 3 ? NULL : video;
	f->encoder.scaled_width = present == 2 ? 0 : 640 + (uint32_t)slot;
	f->encoder.scaled_height = present == 2 ? 0 : 480 + (uint32_t)slot;
	if (slot < MAX_OUTPUT_VIDEO_ENCODERS)
		f->output.video_encoders[slot] = present ? &f->encoder : NULL;
	memcpy(&f->before, &f->output, sizeof(f->before));
	memcpy(&f->encoder_before, &f->encoder, sizeof(f->encoder));
	return f;
}
void oracle_output_size_free(void *handle);
void oracle_output_size_free(void *handle)
{
	free(handle);
}
struct output_size_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_size_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_size_capture(int level, const char *format, va_list args, void *param)
{
	struct output_size_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	char *prefix = strstr(log->message, "oracle_");
	if (prefix)
		memmove(prefix, prefix + 7, strlen(prefix + 7) + 1);
}
typedef uint32_t (*output_size_indexed)(const obs_output_t *, size_t);
typedef uint32_t (*output_size_default)(const obs_output_t *);
typedef void (*output_size_get_log)(log_handler_t *, void **);
typedef void (*output_size_set_log)(log_handler_t, void *);
bool oracle_output_size_probe(void *handle, size_t idx, bool height, bool indexed, output_size_indexed getter,
			      output_size_default wrapper, output_size_get_log get_log, output_size_set_log set_log);
bool oracle_output_size_probe(void *handle, size_t idx, bool height, bool indexed, output_size_indexed getter,
			      output_size_default wrapper, output_size_get_log get_log, output_size_set_log set_log)
{
	struct output_size_fixture *f = handle;
	const obs_output_t *output = f ? &f->output : NULL;
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	struct output_size_log r = {0}, c = {0};
	set_log(output_size_capture, &r);
	uint32_t rv = indexed ? getter(output, idx) : wrapper(output);
	set_log(output_size_capture, &c);
	uint32_t cv = indexed ? (height ? oracle_obs_output_get_height2(output, idx)
				      : oracle_obs_output_get_width2(output, idx))
			      : (height ? oracle_obs_output_get_height(output) : oracle_obs_output_get_width(output));
	set_log(old_handler, old_param);
	return rv == cv && r.count == c.count && r.level == c.level && strcmp(r.message, c.message) == 0 &&
	       (!f || (memcmp(&f->output, &f->before, sizeof(f->output)) == 0 &&
		       memcmp(&f->encoder, &f->encoder_before, sizeof(f->encoder)) == 0));
}
