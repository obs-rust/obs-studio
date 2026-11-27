/* The unchanged C loader and real private session adapter share deterministic
 * dependencies. No FFmpeg layout is copied: include the real private type. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "graphics/graphics.h"
#include "util/threading.h"
enum AVPixelFormat { TFD_NONE = -1, TFD_INITIAL, TFD_DECODED };
typedef struct AVFormatContext AVFormatContext;
typedef struct AVCodecContext AVCodecContext;
typedef struct AVCodec AVCodec;
typedef struct AVFrame AVFrame;
#include "graphics/graphics-ffmpeg-image-private.h"

struct tfd_record {
	const int32_t *config;
	uint32_t *out;
	enum gs_color_format *format;
	uint32_t *width, *height;
	const char *file;
	struct ffmpeg_image *image;
	uint8_t *pixels;
	unsigned events;
	int allocations;
};
static THREAD_LOCAL struct tfd_record *tfd_current;
static void tfd_event(uint32_t event)
{
	struct tfd_record *r = tfd_current;
	if (r->events < 16)
		r->out[8 + r->events] = event;
	r->out[7] = ++r->events;
}
static void tfd_check(bool condition)
{
	if (!condition)
		tfd_current->out[6]++;
}
static void *tfd_alloc(size_t size)
{
	tfd_current->allocations++;
	void *data = calloc(1, size);
	if (!data)
		abort();
	return data;
}
static void tfd_release(void *data)
{
	if (data)
		tfd_current->allocations--;
	free(data);
}
static uint32_t tfd_max_size(void)
{
	return 4096;
}
static bool tfd_check_size(struct ffmpeg_image *image, int cx, int cy)
{
	(void)image;
	(void)cx;
	(void)cy;
	tfd_check(false);
	return false;
}
static void *tfd_convert(struct ffmpeg_image *image, AVFrame *frame, enum gs_image_alpha_mode alpha)
{
	(void)image;
	(void)frame;
	(void)alpha;
	tfd_check(false);
	return NULL;
}
static bool tfd_init(struct ffmpeg_image *image, const char *file, uint32_t (*max_size)(void))
{
	tfd_event(1);
	tfd_check(file == tfd_current->file && max_size == tfd_max_size);
	memset(image, 0, sizeof(*image));
	image->cx = 71;
	image->cy = 72;
	image->format = TFD_INITIAL;
	tfd_current->image = image;
	return tfd_current->config[0] != 0;
}
static void *tfd_decode(void *context, int alpha)
{
	tfd_event(2);
	struct tfd_record *r = tfd_current;
	struct gs_image_decode_context *decode = context;
	tfd_check(decode->image == r->image && decode->check_size == tfd_check_size && decode->convert == tfd_convert);
	tfd_check(alpha == GS_IMAGE_ALPHA_STRAIGHT);
	tfd_check(*r->format == GS_BGRX && *r->width == 91 && *r->height == 92);
	r->image->format = TFD_DECODED;
	r->image->cx = r->config[2];
	r->image->cy = r->config[3];
	if (!r->config[1])
		return NULL;
	static const uint8_t bytes[] = {1, 2, 3, 128, 4, 5, 6, 0};
	r->pixels = tfd_alloc(sizeof(bytes));
	memcpy(r->pixels, bytes, sizeof(bytes));
	return r->pixels;
}
static int tfd_format(int format)
{
	tfd_event(3);
	tfd_check(format == TFD_DECODED);
	return tfd_current->config[4];
}
static void tfd_free(struct ffmpeg_image *image)
{
	tfd_event(4);
	struct tfd_record *r = tfd_current;
	tfd_check(image == r->image);
	bool ok = r->config[1] != 0;
	tfd_check((uint32_t)*r->format == (ok ? (uint32_t)r->config[4] : GS_BGRX));
	tfd_check(*r->width == (ok ? (uint32_t)r->config[2] : 91));
	tfd_check(*r->height == (ok ? (uint32_t)r->config[3] : 92));
}
#define ffmpeg_image_init tfd_init
#define ffmpeg_image_free tfd_free
#define gs_image_max_texture_size tfd_max_size
#define ffmpeg_image_too_large tfd_check_size
#define ffmpeg_image_reformat_frame tfd_convert
#define gs_ffmpeg_image_decode tfd_decode
#define gs_ffmpeg_image_color_format tfd_format
#define bmalloc tfd_alloc
#define bzalloc tfd_alloc
#define bfree tfd_release
#include "graphics-texture-file-data-adapter.inc"
uint8_t *oracle_gs_create_texture_file_data(const char *, enum gs_color_format *, uint32_t *, uint32_t *);
#define gs_create_texture_file_data oracle_gs_create_texture_file_data
#include "graphics-texture-file-data.inc"
#undef gs_create_texture_file_data
#undef bfree
#undef bzalloc
#undef bmalloc
#undef gs_ffmpeg_image_color_format
#undef gs_ffmpeg_image_decode
#undef ffmpeg_image_reformat_frame
#undef ffmpeg_image_too_large
#undef gs_image_max_texture_size
#undef ffmpeg_image_free
#undef ffmpeg_image_init

typedef uint8_t *(*tfd_loader)(const char *, enum gs_color_format *, uint32_t *, uint32_t *);
/* Thread-local recording of callback identities, ordering, publication, pixels
 * and balanced ownership. Passing NULL selects the original C oracle. */
void oracle_texture_file_data_run(tfd_loader loader, const int32_t *config, uint32_t *out);
void oracle_texture_file_data_run(tfd_loader loader, const int32_t *config, uint32_t *out)
{
	enum gs_color_format format = GS_BGRX;
	uint32_t width = 91, height = 92;
	const char file[] = "legacy-image.png";
	memset(out, 0, 24 * sizeof(*out));
	struct tfd_record record = {config, out, &format, &width, &height, file, NULL, NULL, 0, 0};
	tfd_current = &record;
	uint8_t *result = (loader ? loader : oracle_gs_create_texture_file_data)(file, &format, &width, &height);
	out[0] = (uint32_t)format;
	out[1] = width;
	out[2] = height;
	out[3] = result != NULL;
	out[4] = result == record.pixels;
	if (result) {
		static const uint8_t bytes[] = {1, 2, 3, 128, 4, 5, 6, 0};
		tfd_check(memcmp(result, bytes, sizeof(bytes)) == 0);
		tfd_release(result);
	}
	out[5] = (uint32_t)record.allocations;
	tfd_current = NULL;
}
