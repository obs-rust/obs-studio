/* Original extracted C, real private-layout adapter and whole-object parity. */
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define obs_output_set_media oracle_obs_output_set_media
#include "obs-output-set-media-support.c.h"
#include "obs-output-set-media.c.h"
#undef obs_output_set_media
#include "output_set_media_fields.inc"

void oracle_output_set_media_layout(size_t *layout);
void oracle_output_set_media_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_set_media_fields);
	layout[1] = _Alignof(struct obs_output_set_media_fields);
	layout[2] = offsetof(struct obs_output_set_media_fields, flags);
	layout[3] = offsetof(struct obs_output_set_media_fields, video);
	layout[4] = offsetof(struct obs_output_set_media_fields, audio);
	layout[5] = offsetof(struct obs_output_set_media_fields, name);
}
struct output_set_media_log {
	unsigned calls;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_set_media_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_set_media_capture(int level, const char *format, va_list args, void *param)
{
	struct output_set_media_log *log = param;
	log->calls++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
	/* Renaming the oracle symbol changes __func__; restore the API spelling. */
	char *prefix = strstr(log->message, "oracle_obs_output_set_media");
	if (prefix)
		memmove(prefix, prefix + 7, strlen(prefix + 7) + 1);
}
typedef void (*output_set_media_call)(obs_output_t *, video_t *, audio_t *);
typedef void (*output_set_media_get_log)(log_handler_t *, void **);
typedef void (*output_set_media_set_log)(log_handler_t, void *);
bool oracle_output_set_media_check(output_set_media_call call, output_set_media_get_log get_log,
				   output_set_media_set_log set_log, uint32_t flags, bool null_output,
				   unsigned choices);
bool oracle_output_set_media_check(output_set_media_call call, output_set_media_get_log get_log,
				   output_set_media_set_log set_log, uint32_t flags, bool null_output, unsigned choices)
{
	int tokens[4];
	struct obs_output rust_output = {0}, c_output;
	rust_output.context.name = "media-test";
	rust_output.info.flags = flags;
	rust_output.owns_info_id = true;
	rust_output.video = (video_t *)&tokens[0];
	rust_output.audio = (audio_t *)&tokens[1];
	memcpy(&c_output, &rust_output, sizeof(c_output));
	struct obs_output_set_media_fields fields = obs_output_set_media_fields(&rust_output);
	bool equal = fields.flags == &rust_output.info.flags && fields.video == &rust_output.video &&
		     fields.audio == &rust_output.audio && fields.name == rust_output.context.name;
	log_handler_t old;
	void *param;
	get_log(&old, &param);
	for (unsigned clear = 0; clear < 2; clear++) {
		video_t *video = (clear || (choices & 1)) ? NULL : (video_t *)&tokens[2];
		audio_t *audio = (clear || (choices & 2)) ? NULL : (audio_t *)&tokens[3];
		struct output_set_media_log r = {0}, c = {0};
		set_log(output_set_media_capture, &r);
		call(null_output ? NULL : &rust_output, video, audio);
		set_log(output_set_media_capture, &c);
		oracle_obs_output_set_media(null_output ? NULL : &c_output, video, audio);
		equal = equal && memcmp(&rust_output, &c_output, sizeof(c_output)) == 0 && r.calls == c.calls &&
			r.level == c.level && strcmp(r.message, c.message) == 0;
		if (null_output)
			equal = equal && r.calls == 1 && r.level == LOG_DEBUG &&
				strcmp(r.message, "obs_output_set_media: Null 'output' parameter") == 0;
		else if (flags & OBS_OUTPUT_ENCODED)
			equal = equal && r.calls == 1 && r.level == LOG_WARNING &&
				strcmp(r.message,
				       "Output 'media-test': Tried to use obs_output_set_media on an encoded output") ==
					0;
		else
			equal = equal && r.calls == 0;
	}
	set_log(old, param);
	return equal;
}
