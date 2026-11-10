/* Original lifecycle and the real private adapter over deterministic FFmpeg
 * stand-ins. No FFmpeg development headers or new Rust dependencies are needed.
 * Thread-local fixtures keep independent parity cases safe under libtest. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <stdarg.h>
#include <errno.h>
#include <inttypes.h>

#if defined(_MSC_VER)
#define IMAGE_INIT_TLS __declspec(thread)
#else
#define IMAGE_INIT_TLS _Thread_local
#endif
#define AVMEDIA_TYPE_VIDEO 0
#define AV_CODEC_ID_NONE 0
#define AVERROR(e) (-(e))
#define LOG_WARNING 300

enum AVPixelFormat { IMAGE_INIT_FORMAT = 0 };
enum gs_image_alpha_mode { GS_IMAGE_ALPHA_STRAIGHT };
typedef struct AVFrame {
	int unused;
} AVFrame;
typedef struct AVCodec {
	int unused;
} AVCodec;
typedef struct AVCodecParameters {
	int codec_type, codec_id, width, height;
} AVCodecParameters;
typedef struct AVStream {
	AVCodecParameters *codecpar;
} AVStream;
typedef struct AVFormatContext {
	unsigned nb_streams;
	AVStream **streams;
} AVFormatContext;
typedef struct AVCodecContext {
	int width, height;
	void *opaque;
	int (*get_buffer2)(struct AVCodecContext *, AVFrame *, int);
} AVCodecContext;
#include "graphics/graphics-ffmpeg-init-private.h"

struct image_init_fixture {
	const int32_t *config;
	int32_t *out;
	size_t count;
	AVCodecParameters pars[2];
	AVStream streams[2];
	AVStream *pstreams[2];
	AVFormatContext fmt;
	AVCodecContext codec;
	AVCodec decoder;
};
static IMAGE_INIT_TLS struct image_init_fixture image_init_test;
static void image_init_event(int32_t value)
{
	/* Bound every write even if a mutation loops or adds unexpected calls. */
	if (image_init_test.count < 120)
		image_init_test.out[8 + image_init_test.count++] = value;
}
static uint32_t gs_image_max_texture_size(void)
{
	image_init_event(1);
	return 16;
}
static int avformat_open_input(AVFormatContext **out, const char *file, void *fmt, void *opts)
{
	(void)file;
	(void)fmt;
	(void)opts;
	image_init_event(2);
	if (image_init_test.config[6] == 1) {
		*out = NULL;
		return -7;
	}
	*out = &image_init_test.fmt;
	return 0;
}
static int avformat_find_stream_info(AVFormatContext *fmt, void *opts)
{
	(void)fmt;
	(void)opts;
	image_init_event(3);
	return image_init_test.config[6] == 2 ? -8 : 0;
}
static int av_find_best_stream(AVFormatContext *fmt, int type, int wanted, int related, void *decoder, int flags)
{
	(void)decoder;
	image_init_event(4);
	image_init_event(type);
	image_init_event(wanted);
	image_init_event(related);
	image_init_event(flags);
	if (image_init_test.config[6] == 3 || fmt->nb_streams == 0)
		return -9;
	return (int)fmt->nb_streams - 1;
}
static const AVCodec *avcodec_find_decoder(int id)
{
	image_init_event(5);
	image_init_event(id);
	return image_init_test.config[6] == 4 ? NULL : &image_init_test.decoder;
}
static AVCodecContext *avcodec_alloc_context3(const AVCodec *decoder)
{
	(void)decoder;
	image_init_event(6);
	return &image_init_test.codec;
}
static int avcodec_parameters_to_context(AVCodecContext *ctx, const AVCodecParameters *par)
{
	image_init_event(7);
	ctx->width = par->width;
	ctx->height = par->height;
	return image_init_test.config[11]; /* Characterized: original ignores this result. */
}
static int avcodec_default_get_buffer2(AVCodecContext *ctx, AVFrame *frame, int flags)
{
	(void)frame;
	image_init_event(8);
	image_init_event(ctx == &image_init_test.codec);
	image_init_event(ctx->width);
	image_init_event(ctx->height);
	image_init_event(flags);
	return image_init_test.config[10];
}
static int avcodec_open2(AVCodecContext *ctx, const AVCodec *decoder, void *opts)
{
	(void)ctx;
	(void)decoder;
	(void)opts;
	image_init_event(9);
	return image_init_test.config[6] == 5 ? -10 : 0;
}
static void avcodec_free_context(AVCodecContext **ctx)
{
	image_init_event(10);
	*ctx = NULL;
}
static void avformat_close_input(AVFormatContext **fmt)
{
	image_init_event(11);
	*fmt = NULL;
}
static const char *av_err2str(int err)
{
	(void)err;
	return "mock FFmpeg error";
}
#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 1, 2)))
#endif
static void image_init_log(const char *format, ...)
{
	char text[512];
	va_list args;
	va_start(args, format);
	vsnprintf(text, sizeof(text), format, args);
	va_end(args);
	image_init_event(12);
	uint32_t hash = 2166136261u;
	for (size_t i = 0; text[i] && i < sizeof(text) - 1; ++i)
		hash = (hash ^ (unsigned char)text[i]) * 16777619u;
	image_init_event((int32_t)(hash & INT32_MAX));
}
#define blog(level, ...) image_init_log(__VA_ARGS__)
bool oracle_gs_ffmpeg_image_too_large(uint32_t max_size, int cx, int cy);
#define gs_ffmpeg_image_too_large oracle_gs_ffmpeg_image_too_large

/* The reference function bodies are extracted without their PCH, unchanged. */
bool oracle_ffmpeg_image_init(struct ffmpeg_image *info, const char *file, uint32_t (*max_size)(void));
void oracle_ffmpeg_image_free(struct ffmpeg_image *info);
#define ffmpeg_image_init oracle_ffmpeg_image_init
#define ffmpeg_image_free oracle_ffmpeg_image_free
#include "graphics-ffmpeg-init.inc"
#undef ffmpeg_image_init
#undef ffmpeg_image_free

/* This is the real production adapter, not a second implementation. */
#include "graphics-ffmpeg-init-adapter.inc"

void oracle_image_init_run(const int32_t *config, int32_t *out, bool rust);
void oracle_image_init_run(const int32_t *config, int32_t *out, bool rust)
{
	memset(&image_init_test, 0, sizeof(image_init_test));
	memset(out, 0, 128 * sizeof(*out));
	image_init_test.config = config;
	image_init_test.out = out;
	for (size_t i = 0; i < 2; ++i) {
		image_init_test.pars[i] = (AVCodecParameters){config[2], config[3], config[4], config[5]};
		image_init_test.streams[i].codecpar = &image_init_test.pars[i];
		image_init_test.pstreams[i] = &image_init_test.streams[i];
	}
	image_init_test.fmt = (AVFormatContext){(unsigned)config[1], image_init_test.pstreams};
	struct ffmpeg_image info = {0};
	info.max_size = 77;
	unsigned char before[sizeof(info)];
	memcpy(before, &info, sizeof(info));
	const char *path = config[0] == 0 ? NULL : config[0] == 1 ? "" : "image.png";
	bool success = rust ? ffmpeg_image_init(&info, path, gs_image_max_texture_size)
			    : oracle_ffmpeg_image_init(&info, path, gs_image_max_texture_size);
	out[0] = success;
	out[1] = info.fmt_ctx != NULL;
	out[2] = info.decoder_ctx != NULL;
	out[3] = info.too_large;
	out[4] = (int32_t)info.max_size;
	out[5] = config[0] < 2 ? memcmp(before, &info, sizeof(info)) == 0 : 0;
	if (success) {
		AVFrame frame = {0};
		AVCodecContext clone = *info.decoder_ctx;
		AVCodecContext *callback_ctx = config[12] ? &clone : info.decoder_ctx;
		callback_ctx->width = config[7];
		callback_ctx->height = config[8];
		out[6] = callback_ctx->get_buffer2(callback_ctx, &frame, config[9]);
		out[3] = info.too_large;
		if (rust)
			ffmpeg_image_free(&info);
		else
			oracle_ffmpeg_image_free(&info);
		/* Repeated cleanup follows FFmpeg's null-safe free contract. */
		if (rust)
			ffmpeg_image_free(&info);
		else
			oracle_ffmpeg_image_free(&info);
	}
	out[7] = (int32_t)image_init_test.count;
}
#undef gs_ffmpeg_image_too_large
#undef blog
#undef LOG_WARNING
#undef AVERROR
#undef AV_CODEC_ID_NONE
#undef AVMEDIA_TYPE_VIDEO
#undef IMAGE_INIT_TLS
