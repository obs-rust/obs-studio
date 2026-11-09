/* Original C setters plus the real adapter, with deterministic video-call probes.
 * These probes test orchestration only; actual video semantics remain covered
 * by unchanged actual-library Tier 2 tests and media-io's differential suite. */
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define obs_encoder_set_scaled_size oracle_obs_encoder_set_scaled_size
#define obs_encoder_set_gpu_scale_type oracle_obs_encoder_set_gpu_scale_type
#define obs_encoder_set_frame_rate_divisor oracle_obs_encoder_set_frame_rate_divisor
#define obs_encoder_get_name oracle_configuration_name
#define obs_encoder_size_scaled_gate oracle_obs_encoder_size_scaled_gate
#define obs_encoder_size_scaled_match oracle_obs_encoder_size_scaled_match
#define obs_encoder_audio_start_not_video oracle_obs_encoder_audio_start_not_video
#define obs_encoder_audio_start_divisor_ok oracle_obs_encoder_audio_start_divisor_ok
#define video_output_get_info oracle_configuration_info
#define video_output_free_frame_rate_divisor oracle_configuration_free
#define video_output_create_with_frame_rate_divisor oracle_configuration_create
#include "obs-encoder-configuration.c.h"
#undef obs_encoder_get_name
#undef obs_encoder_set_scaled_size
#undef obs_encoder_set_gpu_scale_type
#undef obs_encoder_set_frame_rate_divisor
#include "encoder_configuration_fields.inc"
#undef video_output_get_info
#undef video_output_free_frame_rate_divisor
#undef video_output_create_with_frame_rate_divisor

const char *oracle_configuration_name(const obs_encoder_t *encoder)
{
	return encoder->context.name;
}
struct configuration_trace {
	unsigned info_calls, free_calls, create_calls;
	uint32_t created_divisor;
	video_t *freed;
	video_t *parent;
	unsigned log_count;
	int level;
	char message[256];
};
static struct configuration_trace configuration_trace;
static int configuration_child;
const struct video_output_info *oracle_configuration_info(const video_t *media)
{
	configuration_trace.info_calls++;
	return (const struct video_output_info *)media;
}
void oracle_configuration_free(video_t *video)
{
	configuration_trace.free_calls++;
	configuration_trace.freed = video;
}
video_t *oracle_configuration_create(video_t *media, uint32_t divisor)
{
	configuration_trace.create_calls++;
	configuration_trace.parent = media;
	configuration_trace.created_divisor = divisor;
	return divisor <= 1 ? NULL : (video_t *)&configuration_child;
}
#if defined(__GNUC__) || defined(__clang__)
static void configuration_log(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void configuration_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	configuration_trace.log_count++;
	configuration_trace.level = level;
	vsnprintf(configuration_trace.message, sizeof(configuration_trace.message), format, args);
}

typedef void (*configuration_size_fn)(obs_encoder_t *, uint32_t, uint32_t);
typedef void (*configuration_gpu_fn)(obs_encoder_t *, enum obs_scale_type);
typedef bool (*configuration_divisor_fn)(obs_encoder_t *, uint32_t);
bool oracle_encoder_configuration_probe(unsigned flags, int operation, uint32_t a, uint32_t b, uint32_t output_width,
					uint32_t output_height, configuration_size_fn size_fn,
					configuration_gpu_fn gpu_fn, configuration_divisor_fn divisor_fn);
bool oracle_encoder_configuration_probe(unsigned flags, int operation, uint32_t a, uint32_t b, uint32_t output_width,
					uint32_t output_height, configuration_size_fn size_fn,
					configuration_gpu_fn gpu_fn, configuration_divisor_fn divisor_fn)
{
	struct video_output_info info = {0};
	info.width = output_width;
	info.height = output_height;
	int old_override = 0;
	struct obs_encoder rust = {0}, original = {0};
	rust.context.name = "configuration-test";
	rust.info.type = (flags & 1) ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
	rust.orig_info.type = (flags & 1) ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
	rust.active = (flags & 2) != 0;
	rust.initialized = (flags & 4) != 0;
	rust.scaled_width = UINT32_C(123456);
	rust.scaled_height = UINT32_C(654321);
	rust.gpu_scale_type = OBS_SCALE_BICUBIC;
	rust.frame_rate_divisor = 7;
	rust.media = (flags & 8) ? &info : NULL;
	rust.fps_override = (flags & 16) ? (video_t *)&old_override : NULL;
	memcpy(&original, &rust, sizeof(rust));
	obs_encoder_t *r = (flags & 32) ? NULL : &rust;
	obs_encoder_t *c = (flags & 32) ? NULL : &original;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(configuration_log, NULL);
	memset(&configuration_trace, 0, sizeof(configuration_trace));
	bool rust_result = true, c_result = true;
	if (operation == 0)
		size_fn(r, a, b);
	else if (operation == 1)
		gpu_fn(r, (enum obs_scale_type)a);
	else
		rust_result = divisor_fn(r, a);
	struct configuration_trace rust_trace;
	memcpy(&rust_trace, &configuration_trace, sizeof(rust_trace));
	memset(&configuration_trace, 0, sizeof(configuration_trace));
	if (operation == 0)
		oracle_obs_encoder_set_scaled_size(c, a, b);
	else if (operation == 1)
		oracle_obs_encoder_set_gpu_scale_type(c, (enum obs_scale_type)a);
	else
		c_result = oracle_obs_encoder_set_frame_rate_divisor(c, a);
	base_set_log_handler(old_handler, old_param);
	return rust_result == c_result && memcmp(&rust, &original, sizeof(rust)) == 0 &&
	       memcmp(&rust_trace, &configuration_trace, sizeof(rust_trace)) == 0;
}

void oracle_encoder_configuration_layout(size_t *layout);
void oracle_encoder_configuration_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_configuration_fields);
	layout[1] = _Alignof(struct obs_encoder_configuration_fields);
	layout[2] = offsetof(struct obs_encoder_configuration_fields, kind);
	layout[3] = offsetof(struct obs_encoder_configuration_fields, active);
	layout[4] = offsetof(struct obs_encoder_configuration_fields, initialized);
	layout[5] = offsetof(struct obs_encoder_configuration_fields, width);
	layout[6] = offsetof(struct obs_encoder_configuration_fields, height);
	layout[7] = offsetof(struct obs_encoder_configuration_fields, gpu);
	layout[8] = offsetof(struct obs_encoder_configuration_fields, divisor);
	layout[9] = offsetof(struct obs_encoder_configuration_fields, media);
	layout[10] = offsetof(struct obs_encoder_configuration_fields, fps_override);
	layout[11] = offsetof(struct obs_encoder_configuration_fields, name);
	layout[12] = sizeof(enum obs_encoder_type);
	layout[13] = sizeof(enum obs_scale_type);
	layout[14] = sizeof(bool);
}
