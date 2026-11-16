/* Original extracted decoder and the real private adapter over deterministic
 * FFmpeg operations. All state is per call, including the packet's owner. */
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <stdarg.h>
#include <errno.h>

#define AVERROR(x) (-(x))
#define AVERROR_EOF (-100)
#define LOG_WARNING 200
#define av_err2str(x) image_decode_error(x)
enum gs_image_alpha_mode { GS_IMAGE_ALPHA_STRAIGHT };
enum AVPixelFormat { PIXEL_NONE };
struct scenario {
	const int *config;
	int *out;
	size_t event_count, status_at;
	const void *packet;
};
typedef struct {
	struct scenario *owner;
} AVFormatContext;
typedef struct {
	struct scenario *owner;
} AVCodecContext;
typedef struct {
	struct scenario *owner;
} AVPacket;
typedef struct {
	int width, height, format;
	struct scenario *owner;
} AVFrame;
#include "graphics/graphics-ffmpeg-image-private.h"

/* av_frame_alloc has no argument: each exported call below owns a TLS scenario.
 * Concurrent property tests cannot overwrite another thread's state. */
#ifdef _MSC_VER
__declspec(thread) static struct scenario *current;
#else
static _Thread_local struct scenario *current;
#endif
static const char *image_decode_error(int error)
{
	current->out[124] = error;
	return "error";
}
static void event(struct scenario *s, int value)
{
	if (s->event_count < 116)
		s->out[8 + s->event_count++] = value;
	s->out[7] = (int)s->event_count;
}
static int next_status(struct scenario *s)
{
	int ret = s->status_at < (size_t)s->config[7] ? s->config[8 + s->status_at++] : 0;
	return ret == -11 ? AVERROR(EAGAIN) : ret;
}
static AVFrame *av_frame_alloc(void)
{
	event(current, 1);
	if (!current->config[0])
		return NULL;
	AVFrame *frame = calloc(1, sizeof(*frame));
	if (!frame)
		abort();
	frame->owner = current;
	frame->width = current->config[4];
	frame->height = current->config[5];
	frame->format = current->config[6];
	return frame;
}
static int av_read_frame(AVFormatContext *ctx, AVPacket *packet)
{
	packet->owner = ctx->owner;
	event(ctx->owner, 2);
	return ctx->owner->config[1];
}
static int avcodec_send_packet(AVCodecContext *ctx, const AVPacket *packet)
{
	if (packet->owner != ctx->owner || (ctx->owner->packet && ctx->owner->packet != packet))
		abort();
	ctx->owner->packet = packet;
	event(ctx->owner, 3);
	return next_status(ctx->owner);
}
static int avcodec_receive_frame(AVCodecContext *ctx, AVFrame *frame)
{
	if (frame->owner != ctx->owner)
		abort();
	event(ctx->owner, 4);
	return next_status(ctx->owner);
}
static void av_packet_unref(AVPacket *packet)
{
	event(packet->owner, 13);
}
static void av_frame_free(AVFrame **frame)
{
	event((*frame)->owner, 14);
	free(*frame);
	*frame = NULL;
}
static void *av_mallocz(size_t size)
{
	void *data = calloc(1, size);
	if (!data)
		abort();
	return data;
}
static void av_free(void *data)
{
	free(data);
}
/* This stand-in records the exact original warning format and error argument. */
static void blog(int level, const char *format, ...)
{
	(void)level;
	int kind = format[10] == 'c' ? 10 : format[10] == 'r' ? 11 : 12;
	event(current, kind);
}
static bool ffmpeg_image_too_large(struct ffmpeg_image *info, int width, int height)
{
	struct scenario *s = info->fmt_ctx->owner;
	s->out[125] = width;
	s->out[126] = height;
	event(s, 6);
	if (s->config[3]) {
		info->too_large = true;
		return true;
	}
	return false;
}
static void *ffmpeg_image_reformat_frame(struct ffmpeg_image *info, AVFrame *frame, enum gs_image_alpha_mode alpha_mode)
{
	struct scenario *s = frame->owner;
	event(s, info->cx == frame->width && info->cy == frame->height && (int)info->format == frame->format ? 7 : 99);
	event(s, 8);
	s->out[6] = (int)alpha_mode;
	return s->config[2] == 2 ? NULL : info;
}
#include "image-decode-original.inc"
#include "image-decode-adapter.inc"

typedef void *(*decode_fn)(void *, int);
void oracle_image_decode_run(const int *config, int *out, decode_fn rust_decode);
void oracle_image_decode_run(const int *config, int *out, decode_fn rust_decode)
{
	struct scenario s = {config, out, 0, 0, NULL};
	current = &s;
	AVFormatContext format = {&s};
	AVCodecContext codec = {&s};
	struct ffmpeg_image info = {.file = "image",
				    .fmt_ctx = &format,
				    .decoder_ctx = &codec,
				    .cx = 123,
				    .cy = 456,
				    .format = PIXEL_NONE,
				    .too_large = config[2] == 1};
	struct gs_image_decode_context context = {&info, ffmpeg_image_too_large, ffmpeg_image_reformat_frame};
	void *data = rust_decode ? rust_decode(&context, config[24])
				 : ffmpeg_image_decode(&info, (enum gs_image_alpha_mode)config[24]);
	out[0] = data != NULL;
	out[1] = info.cx;
	out[2] = info.cy;
	out[3] = (int)info.format;
	out[4] = info.too_large;
	out[5] = (int)s.status_at;
	current = NULL;
}
