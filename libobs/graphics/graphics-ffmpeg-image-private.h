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
};

/* Only C knows these callbacks and layouts. Rust borrows this context opaquely. */
struct gs_image_decode_context {
	struct ffmpeg_image *image;
	bool (*check_size)(struct ffmpeg_image *, int, int);
	void *(*convert)(struct ffmpeg_image *, AVFrame *, enum gs_image_alpha_mode);
};
