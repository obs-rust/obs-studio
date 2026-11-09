/* Private media-unity state. FFmpeg types never cross the Rust ABI. */
#pragma once

struct ffmpeg_image {
	const char *file;
	AVFormatContext *fmt_ctx;
	AVCodecContext *decoder_ctx;

	int cx, cy;
	enum AVPixelFormat format;

	/* The texture size limit of the graphics device, 0 if unknown. */
	uint32_t max_size;
	bool too_large;

	/* Scratch decoder lookup used only by the private Rust adapter. */
	const AVCodec *init_decoder;
	int (*get_buffer)(struct ffmpeg_image *, void *, int);
};

bool ffmpeg_image_too_large(struct ffmpeg_image *info, int cx, int cy);
bool ffmpeg_image_init(struct ffmpeg_image *info, const char *file, uint32_t (*max_size)(void));
void ffmpeg_image_free(struct ffmpeg_image *info);
int obs_ffmpeg_image_get_buffer(struct ffmpeg_image *info, void *buffer, int flags);
int obs_ffmpeg_image_operation(struct ffmpeg_image *info, uint32_t op, int value, void *extra,
			       int (*get_buffer)(struct ffmpeg_image *, void *, int), uint32_t (*max_size)(void));
