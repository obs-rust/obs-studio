/* Original C functions, real layout, and production adapters for parity. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_set_video_conversion oracle_obs_output_set_video_conversion
#define obs_output_set_audio_conversion oracle_obs_output_set_audio_conversion
#define obs_output_add_packet_callback oracle_obs_output_add_packet_callback
#define obs_output_remove_packet_callback oracle_obs_output_remove_packet_callback
#include "obs-output-conversion-setters.c.h"
#include "obs-output-packet-callbacks.c.h"
#undef obs_output_set_video_conversion
#undef obs_output_set_audio_conversion
#undef obs_output_add_packet_callback
#undef obs_output_remove_packet_callback
#include "output_conversion_callbacks_fields.inc"

struct conversion_log {
	unsigned count;
	int level;
	char text[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void conversion_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void conversion_capture(int level, const char *format, va_list args, void *param)
{
	struct conversion_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
	/* __func__ in renamed oracle setters contains this prefix. */
	char *prefix = strstr(log->text, "oracle_obs_output_");
	if (prefix)
		memmove(prefix, prefix + 7, strlen(prefix + 7) + 1);
}
static void conversion_packet(obs_output_t *output, struct encoder_packet *pkt, struct encoder_packet_time *time,
			      void *param)
{
	(void)output;
	(void)pkt;
	(void)time;
	(void)param;
	abort(); /* Registration must never invoke the callback. */
}
static void conversion_other_packet(obs_output_t *output, struct encoder_packet *pkt, struct encoder_packet_time *time,
				    void *param)
{
	conversion_packet(output, pkt, time, param);
}
typedef void (*conversion_video_fn)(obs_output_t *, const struct video_scale_info *);
typedef void (*conversion_audio_fn)(obs_output_t *, const struct audio_convert_info *);
typedef void (*conversion_packet_fn)(obs_output_t *,
				     void (*)(obs_output_t *, struct encoder_packet *, struct encoder_packet_time *,
					      void *),
				     void *);
typedef void (*conversion_get_log_fn)(log_handler_t *, void **);
typedef void (*conversion_set_log_fn)(log_handler_t, void *);

bool oracle_output_conversion_callbacks_check(conversion_video_fn video, conversion_audio_fn audio,
					      conversion_packet_fn add, conversion_packet_fn remove,
					      conversion_get_log_fn get_log, conversion_set_log_fn set_log,
					      uint32_t flags, uint32_t value, uint32_t operations);
bool oracle_output_conversion_callbacks_check(conversion_video_fn video, conversion_audio_fn audio,
					      conversion_packet_fn add, conversion_packet_fn remove,
					      conversion_get_log_fn get_log, conversion_set_log_fn set_log,
					      uint32_t flags, uint32_t value, uint32_t operations)
{
	struct obs_output r = {0}, c = {0};
	r.info.flags = c.info.flags = flags;
	r.context.name = c.context.name = "fixture";
	pthread_mutex_init(&r.pkt_callbacks_mutex, NULL);
	pthread_mutex_init(&c.pkt_callbacks_mutex, NULL);
	log_handler_t old;
	void *old_param;
	get_log(&old, &old_param);
	bool equal = true;
	for (unsigned pass = 0; pass < 6; pass++) {
		struct video_scale_info v = {.format = VIDEO_FORMAT_NV12,
					     .width = value,
					     .height = value ^ 0xabc,
					     .range = VIDEO_RANGE_FULL,
					     .colorspace = VIDEO_CS_709};
		struct audio_convert_info a = {.samples_per_sec = value,
					       .format = AUDIO_FORMAT_FLOAT_PLANAR,
					       .speakers = SPEAKERS_5POINT1,
					       .allow_clipping = (value & 1) != 0};
		if (pass == 5) {
			memset(&v, 0, sizeof(v));
			memset(&a, 0, sizeof(a));
		}
		for (unsigned kind = 0; kind < 2; kind++) {
			struct conversion_log rl = {0}, cl = {0};
			set_log(conversion_capture, &rl);
			if (kind == 0)
				video(pass < 2 ? NULL : &r, pass % 2 ? &v : NULL);
			else
				audio(pass < 2 ? NULL : &r, pass % 2 ? &a : NULL);
			set_log(conversion_capture, &cl);
			if (kind == 0)
				oracle_obs_output_set_video_conversion(pass < 2 ? NULL : &c, pass % 2 ? &v : NULL);
			else
				oracle_obs_output_set_audio_conversion(pass < 2 ? NULL : &c, pass % 2 ? &a : NULL);
			equal = equal && rl.count == cl.count && rl.level == cl.level && strcmp(rl.text, cl.text) == 0;
		}
		equal = equal && r.video_conversion_set == c.video_conversion_set &&
			r.audio_conversion_set == c.audio_conversion_set &&
			r.video_conversion.format == c.video_conversion.format &&
			r.video_conversion.width == c.video_conversion.width &&
			r.video_conversion.height == c.video_conversion.height &&
			r.video_conversion.range == c.video_conversion.range &&
			r.video_conversion.colorspace == c.video_conversion.colorspace &&
			r.audio_conversion.samples_per_sec == c.audio_conversion.samples_per_sec &&
			r.audio_conversion.format == c.audio_conversion.format &&
			r.audio_conversion.speakers == c.audio_conversion.speakers &&
			r.audio_conversion.allow_clipping == c.audio_conversion.allow_clipping;
	}
	for (unsigned i = 0; i < 32; i++) {
		uint32_t op = (operations >> (i % 8 * 4)) & 15;
		void (*callback)(obs_output_t *, struct encoder_packet *, struct encoder_packet_time *, void *) =
			(op & 2)   ? conversion_packet
			: (op & 4) ? conversion_other_packet
				   : NULL;
		void *param = (op & 8) ? &value : NULL;
		if (op & 1) {
			add(&r, callback, param);
			oracle_obs_output_add_packet_callback(&c, callback, param);
		} else {
			remove(&r, callback, param);
			oracle_obs_output_remove_packet_callback(&c, callback, param);
		}
		equal = equal && r.pkt_callbacks.num == c.pkt_callbacks.num;
		for (size_t j = 0; equal && j < r.pkt_callbacks.num; j++)
			equal = r.pkt_callbacks.array[j].packet_cb == c.pkt_callbacks.array[j].packet_cb &&
				r.pkt_callbacks.array[j].param == c.pkt_callbacks.array[j].param;
	}
	set_log(old, old_param);
	da_free(r.pkt_callbacks);
	da_free(c.pkt_callbacks);
	pthread_mutex_destroy(&r.pkt_callbacks_mutex);
	pthread_mutex_destroy(&c.pkt_callbacks_mutex);
	return equal;
}

void oracle_output_conversion_callbacks_layout(size_t *layout);
void oracle_output_conversion_callbacks_layout(size_t *layout)
{
	size_t values[] = {sizeof(struct video_scale_info),
			   _Alignof(struct video_scale_info),
			   offsetof(struct video_scale_info, format),
			   offsetof(struct video_scale_info, width),
			   offsetof(struct video_scale_info, height),
			   offsetof(struct video_scale_info, range),
			   offsetof(struct video_scale_info, colorspace),
			   sizeof(struct audio_convert_info),
			   _Alignof(struct audio_convert_info),
			   offsetof(struct audio_convert_info, samples_per_sec),
			   offsetof(struct audio_convert_info, format),
			   offsetof(struct audio_convert_info, speakers),
			   offsetof(struct audio_convert_info, allow_clipping),
			   sizeof(struct packet_callback),
			   _Alignof(struct packet_callback),
			   offsetof(struct packet_callback, packet_cb),
			   offsetof(struct packet_callback, param)};
	memcpy(layout, values, sizeof(values));
}
