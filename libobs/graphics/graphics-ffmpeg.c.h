#include "build/libobs-media.pch.h"

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

/* A picture wider or taller than the graphics device's texture size limit
 * cannot become a texture. Say so once, before anything is decoded. */
static bool ffmpeg_image_too_large(struct ffmpeg_image *info, int cx, int cy)
{
	if (!info->max_size || cx <= 0 || cy <= 0 || ((uint32_t)cx <= info->max_size && (uint32_t)cy <= info->max_size))
		return false;

	if (!info->too_large)
		blog(LOG_WARNING,
		     "Image '%s' is %dx%d, larger than the maximum texture size of "
		     "the graphics device (%" PRIu32 "x%" PRIu32 "); not loading it",
		     info->file, cx, cy, info->max_size, info->max_size);
	info->too_large = true;
	return true;
}

/* The decoder asks for the frame buffer once it has read the picture's
 * header and before it decodes the pixels. */
static int ffmpeg_image_get_buffer(AVCodecContext *ctx, AVFrame *frame, int flags)
{
	if (ffmpeg_image_too_large(ctx->opaque, ctx->width, ctx->height))
		return AVERROR(EINVAL);

	return avcodec_default_get_buffer2(ctx, frame, flags);
}

static bool ffmpeg_image_open_decoder_context(struct ffmpeg_image *info)
{
	AVFormatContext *const fmt_ctx = info->fmt_ctx;
	int ret = av_find_best_stream(fmt_ctx, AVMEDIA_TYPE_VIDEO, -1, 1, NULL, 0);
	if (ret < 0) {
		blog(LOG_WARNING, "Couldn't find video stream in file '%s': %s", info->file, av_err2str(ret));
		return false;
	}

	AVStream *const stream = fmt_ctx->streams[ret];
	AVCodecParameters *const codecpar = stream->codecpar;
	const AVCodec *const decoder = avcodec_find_decoder(codecpar->codec_id); // fix discarded-qualifiers

	if (!decoder) {
		blog(LOG_WARNING, "Failed to find decoder for file '%s'", info->file);
		return false;
	}

	if (ffmpeg_image_too_large(info, codecpar->width, codecpar->height))
		return false;

	AVCodecContext *const decoder_ctx = avcodec_alloc_context3(decoder);
	avcodec_parameters_to_context(decoder_ctx, codecpar);
	decoder_ctx->opaque = info;
	decoder_ctx->get_buffer2 = ffmpeg_image_get_buffer;

	info->decoder_ctx = decoder_ctx;

	ret = avcodec_open2(decoder_ctx, decoder, NULL);
	if (ret < 0) {
		blog(LOG_WARNING,
		     "Failed to open video codec for file '%s': "
		     "%s",
		     info->file, av_err2str(ret));
		return false;
	}

	return true;
}

static void ffmpeg_image_free(struct ffmpeg_image *info)
{
	avcodec_free_context(&info->decoder_ctx);
	avformat_close_input(&info->fmt_ctx);
}

static bool ffmpeg_image_init(struct ffmpeg_image *info, const char *file)
{
	int ret;

	if (!file || !*file)
		return false;

	memset(info, 0, sizeof(struct ffmpeg_image));
	info->file = file;
	info->max_size = gs_image_max_texture_size();

	ret = avformat_open_input(&info->fmt_ctx, file, NULL, NULL);
	if (ret < 0) {
		blog(LOG_WARNING, "Failed to open file '%s': %s", info->file, av_err2str(ret));
		return false;
	}

	/* For a single picture (PNG, JPEG, WebP, BMP...) the demuxer already
	 * knows the codec. avformat_find_stream_info() would decode the whole
	 * picture once more only to learn its size and pixel format, which the
	 * decode reports anyway, and with no size check. */
	const AVFormatContext *const fmt_ctx = info->fmt_ctx;
	const bool known_codec = fmt_ctx->nb_streams == 1 &&
				 fmt_ctx->streams[0]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO &&
				 fmt_ctx->streams[0]->codecpar->codec_id != AV_CODEC_ID_NONE;
	if (!known_codec) {
		ret = avformat_find_stream_info(info->fmt_ctx, NULL);
		if (ret < 0) {
			blog(LOG_WARNING,
			     "Could not find stream info for file '%s':"
			     " %s",
			     info->file, av_err2str(ret));
			goto fail;
		}
	}

	if (!ffmpeg_image_open_decoder_context(info))
		goto fail;

	return true;

fail:
	ffmpeg_image_free(info);
	return false;
}

/* Portable on every compiler; GCC, Clang and MSVC turn it into one byte swap. */
#define obs_bswap16(v) ((uint16_t)(((uint16_t)(v) << 8) | ((uint16_t)(v) >> 8)))

static void *ffmpeg_image_copy_data_straight(struct ffmpeg_image *info, AVFrame *frame)
{
	const size_t linesize = (size_t)info->cx * 4;
	const size_t totalsize = info->cy * linesize;
	void *data = bmalloc(totalsize);

	const size_t src_linesize = frame->linesize[0];
	if (linesize != src_linesize) {
		const size_t min_line = linesize < src_linesize ? linesize : src_linesize;

		uint8_t *dst = data;
		const uint8_t *src = frame->data[0];
		for (int y = 0; y < info->cy; y++) {
			memcpy(dst, src, min_line);
			dst += linesize;
			src += src_linesize;
		}
	} else {
		memcpy(data, frame->data[0], totalsize);
	}

	return data;
}

static inline size_t get_dst_position(const size_t w, const size_t h, const size_t x, const size_t y, int orient)
{
	size_t res_x = 0;
	size_t res_y = 0;

	if (orient == 2) {
		/*
		 * Orientation 2: Flip X
		 *
		 * 888888        888888
		 *     88   ->   88
		 *   8888   ->   8888
		 *     88   ->   88
		 *     88        88
		 *
		 * (0, 0) -> (w, 0)
		 * (0, h) -> (w, h)
		 * (w, 0) -> (0, 0)
		 * (w, h) -> (0, h)
		 *
		 * (w - x, y)
		 */

		res_x = w - 1 - x;
		res_y = y;

	} else if (orient == 3) {
		/*
		 * Orientation 3: 180 degree
		 *
		 *     88        888888
		 *     88   ->   88
		 *   8888   ->   8888
		 *     88   ->   88
		 * 888888        88
		 *
		 * (0, 0) -> (w, h)
		 * (0, h) -> (w, 0)
		 * (w, 0) -> (0, h)
		 * (w, h) -> (0, 0)
		 *
		 * (w - x, h - y)
		 */

		res_x = w - 1 - x;
		res_y = h - 1 - y;

	} else if (orient == 4) {
		/*
		 * Orientation 4: Flip Y
		 *
		 * 88            888888
		 * 88       ->   88
		 * 8888     ->   8888
		 * 88       ->   88
		 * 888888        88
		 *
		 * (0, 0) -> (0, h)
		 * (0, h) -> (0, 0)
		 * (w, 0) -> (w, h)
		 * (w, h) -> (w, 0)
		 *
		 * (x, h - y)
		 */

		res_x = x;
		res_y = h - 1 - y;

	} else if (orient == 5) {
		/*
		 * Orientation 5: Flip Y + 90 degree CW
		 *
		 * 8888888888        888888
		 * 88  88       ->   88
		 * 88           ->   8888
		 *              ->   88
		 *                   88
		 *
		 * (0, 0) -> (0, 0)
		 * (0, h) -> (w, 0)
		 * (w, 0) -> (0, h)
		 * (w, h) -> (w, h)
		 *
		 * (y, x)
		 */

		res_x = y;
		res_y = x;

	} else if (orient == 6) {
		/*
		 * Orientation 6: 90 degree CW
		 *
		 * 88                888888
		 * 88  88       ->   88
		 * 8888888888   ->   8888
		 *              ->   88
		 *                   88
		 *
		 * (0, 0) -> (w, 0)
		 * (0, h) -> (0, 0)
		 * (w, 0) -> (w, h)
		 * (w, h) -> (0, h)
		 *
		 * (w - y, x)
		 */

		res_x = w - 1 - y;
		res_y = x;

	} else if (orient == 7) {
		/*
		 * Orientation 7: Flip Y + 90 degree CCW
		 *
		 *         88        888888
		 *     88  88   ->   88
		 * 8888888888   ->   8888
		 *              ->   88
		 *                   88
		 *
		 * (0, 0) -> (w, h)
		 * (0, h) -> (0, h)
		 * (w, 0) -> (w, 0)
		 * (w, h) -> (0, 0)
		 *
		 * (w - y, h - x)
		 */

		res_x = w - 1 - y;
		res_y = h - 1 - x;

	} else if (orient == 8) {
		/*
		 * Orientation 8: 90 degree CCW
		 *
		 * 8888888888        888888
		 *     88  88   ->   88
		 *         88   ->   8888
		 *              ->   88
		 *                   88
		 *
		 * (0, 0) -> (0, h)
		 * (0, h) -> (w, h)
		 * (w, 0) -> (0, 0)
		 * (w, h) -> (w, 0)
		 *
		 * (y, h - x)
		 */

		res_x = y;
		res_y = h - 1 - x;
	}

	return (res_x + res_y * w) * 4;
}

#define TILE_SIZE 16
#define MIN(a, b) (((a) < (b)) ? (a) : (b))

static void *ffmpeg_image_orient(struct ffmpeg_image *info, void *in_data, int orient)
{
	const size_t sx = (size_t)info->cx;
	const size_t sy = (size_t)info->cy;
	uint8_t *data = NULL;

	if (orient == 0 || orient == 1)
		return in_data;

	data = bmalloc(sx * 4 * sy);

	if (orient >= 5 && orient < 9) {
		info->cx = (int)sy;
		info->cy = (int)sx;
	}

	uint8_t *src = in_data;

	size_t off_dst;
	size_t off_src = 0;
	for (size_t y0 = 0; y0 < sy; y0 += TILE_SIZE) {
		for (size_t x0 = 0; x0 < sx; x0 += TILE_SIZE) {
			size_t lim_x = MIN((size_t)sx, x0 + TILE_SIZE);
			size_t lim_y = MIN((size_t)sy, y0 + TILE_SIZE);

			for (size_t y = y0; y < lim_y; y++) {
				for (size_t x = x0; x < lim_x; x++) {
					off_src = (x + y * sx) * 4;

					off_dst = get_dst_position(info->cx, info->cy, x, y, orient);

					memcpy(data + off_dst, src + off_src, 4);
				}
			}
		}
	}

	bfree(in_data);
	return data;
}

/* FFmpeg 8.1 (libavcodec 62.13.100) parses the EXIF block of JPEG, PNG and
 * WebP images itself (the av_exif_* API). It exports the orientation only as
 * display matrix side data and removes the "Orientation" tag from the frame
 * metadata, where FFmpeg 8.0 and earlier put it (JPEG and WebP only).
 * Once the minimum FFmpeg is 8.1, the metadata tag and the sign table below
 * can go. */
#if !FFMPEG_HAS_EXIF_API
static inline int matrix_sign(int32_t v)
{
	return (v > 0) - (v < 0);
}

/* The EXIF orientation (1-8) of a display matrix, for FFmpeg without
 * av_exif_matrix_to_orientation(). */
static int display_matrix_to_orientation(const int32_t *matrix)
{
	/* The display matrix FFmpeg derives from each EXIF orientation, as
	 * the signs of its 2x2 part: {a, b, c, d} = {m[0], m[1], m[3], m[4]}. */
	static const int8_t orientations[8][4] = {
		{1, 0, 0, 1}, {-1, 0, 0, 1}, {-1, 0, 0, -1}, {1, 0, 0, -1},
		{0, 1, 1, 0}, {0, 1, -1, 0}, {0, -1, -1, 0}, {0, -1, 1, 0},
	};
	const int sign[4] = {matrix_sign(matrix[0]), matrix_sign(matrix[1]), matrix_sign(matrix[3]),
			     matrix_sign(matrix[4])};

	for (int i = 0; i < 8; i++) {
		if (sign[0] == orientations[i][0] && sign[1] == orientations[i][1] && sign[2] == orientations[i][2] &&
		    sign[3] == orientations[i][3])
			return i + 1;
	}

	return 0;
}
#endif

/* EXIF orientation (1-8) of a decoded image, 0 if it has none. */
static int ffmpeg_image_get_orientation(const AVFrame *frame)
{
#if !FFMPEG_HAS_EXIF_API
	AVDictionaryEntry *entry = av_dict_get(frame->metadata, "Orientation", NULL, AV_DICT_MATCH_CASE);
	if (entry && entry->value)
		return atoi(entry->value);
#endif

	const AVFrameSideData *sd = av_frame_get_side_data(frame, AV_FRAME_DATA_DISPLAYMATRIX);
	if (!sd || sd->size < 9 * sizeof(int32_t))
		return 0;

	int32_t matrix[9];
	memcpy(matrix, sd->data, sizeof(matrix));
#if FFMPEG_HAS_EXIF_API
	return av_exif_matrix_to_orientation(matrix);
#else
	return display_matrix_to_orientation(matrix);
#endif
}

/* The JPEG decoders output the deprecated full-range YUVJ formats. swscale
 * still takes them, but logs "deprecated pixel format used" for every
 * context made from one, so each decoded JPEG logged it once (#2334). Give
 * swscale the matching YUV format and say the range is full instead; the
 * pixels come out the same. Other formats are passed as they are. */
static enum AVPixelFormat ffmpeg_image_sws_format(enum AVPixelFormat format, bool *full_range)
{
	*full_range = true;
	switch (format) {
	case AV_PIX_FMT_YUVJ420P:
		return AV_PIX_FMT_YUV420P;
	case AV_PIX_FMT_YUVJ422P:
		return AV_PIX_FMT_YUV422P;
	case AV_PIX_FMT_YUVJ444P:
		return AV_PIX_FMT_YUV444P;
	case AV_PIX_FMT_YUVJ440P:
		return AV_PIX_FMT_YUV440P;
	case AV_PIX_FMT_YUVJ411P:
		return AV_PIX_FMT_YUV411P;
	default:
		*full_range = false;
		return format;
	}
}

static void *ffmpeg_image_reformat_frame(struct ffmpeg_image *info, AVFrame *frame, enum gs_image_alpha_mode alpha_mode)
{
	struct SwsContext *sws_ctx = NULL;
	void *data = NULL;
	int ret = 0;

	const int orient = ffmpeg_image_get_orientation(frame);

	if (info->format == AV_PIX_FMT_BGR0) {
		data = ffmpeg_image_copy_data_straight(info, frame);
	} else if (info->format == AV_PIX_FMT_RGBA || info->format == AV_PIX_FMT_BGRA) {
		if (alpha_mode == GS_IMAGE_ALPHA_STRAIGHT) {
			data = ffmpeg_image_copy_data_straight(info, frame);
		} else {
			const size_t linesize = (size_t)info->cx * 4;
			const size_t totalsize = info->cy * linesize;
			data = bmalloc(totalsize);
			const size_t src_linesize = frame->linesize[0];
			const size_t min_line = linesize < src_linesize ? linesize : src_linesize;
			uint8_t *dst = data;
			const uint8_t *src = frame->data[0];
			const size_t row_elements = min_line >> 2;
			if (alpha_mode == GS_IMAGE_ALPHA_PREMULTIPLY_SRGB) {
				for (int y = 0; y < info->cy; y++) {
					gs_premultiply_xyza_srgb_loop_restrict(dst, src, row_elements);
					dst += linesize;
					src += src_linesize;
				}
			} else if (alpha_mode == GS_IMAGE_ALPHA_PREMULTIPLY) {
				for (int y = 0; y < info->cy; y++) {
					gs_premultiply_xyza_loop_restrict(dst, src, row_elements);
					dst += linesize;
					src += src_linesize;
				}
			}
		}
	} else if (info->format == AV_PIX_FMT_RGBA64BE) {
		const size_t dst_linesize = (size_t)info->cx * 4;
		data = bmalloc(info->cy * dst_linesize);
		const size_t src_linesize = frame->linesize[0];
		const size_t src_min_line = (dst_linesize * 2) < src_linesize ? (dst_linesize * 2) : src_linesize;
		const size_t row_elements = src_min_line >> 3;
		uint8_t *dst = data;
		const uint8_t *src = frame->data[0];
		uint16_t value[4];
		float f[4];
		if (alpha_mode == GS_IMAGE_ALPHA_STRAIGHT) {
			for (int y = 0; y < info->cy; y++) {
				for (size_t x = 0; x < row_elements; ++x) {
					memcpy(value, src, sizeof(value));
					f[0] = (float)obs_bswap16(value[0]) / 65535.0f;
					f[1] = (float)obs_bswap16(value[1]) / 65535.0f;
					f[2] = (float)obs_bswap16(value[2]) / 65535.0f;
					f[3] = (float)obs_bswap16(value[3]) / 65535.0f;
					gs_float4_to_u8x4(dst, f);
					dst += sizeof(*dst) * 4;
					src += sizeof(value);
				}

				src += src_linesize - src_min_line;
			}
		} else if (alpha_mode == GS_IMAGE_ALPHA_PREMULTIPLY_SRGB) {
			for (int y = 0; y < info->cy; y++) {
				for (size_t x = 0; x < row_elements; ++x) {
					memcpy(value, src, sizeof(value));
					f[0] = (float)obs_bswap16(value[0]) / 65535.0f;
					f[1] = (float)obs_bswap16(value[1]) / 65535.0f;
					f[2] = (float)obs_bswap16(value[2]) / 65535.0f;
					f[3] = (float)obs_bswap16(value[3]) / 65535.0f;
					gs_float3_srgb_nonlinear_to_linear(f);
					gs_premultiply_float4(f);
					gs_float3_srgb_linear_to_nonlinear(f);
					gs_float4_to_u8x4(dst, f);
					dst += sizeof(*dst) * 4;
					src += sizeof(value);
				}

				src += src_linesize - src_min_line;
			}
		} else if (alpha_mode == GS_IMAGE_ALPHA_PREMULTIPLY) {
			for (int y = 0; y < info->cy; y++) {
				for (size_t x = 0; x < row_elements; ++x) {
					memcpy(value, src, sizeof(value));
					f[0] = (float)obs_bswap16(value[0]) / 65535.0f;
					f[1] = (float)obs_bswap16(value[1]) / 65535.0f;
					f[2] = (float)obs_bswap16(value[2]) / 65535.0f;
					f[3] = (float)obs_bswap16(value[3]) / 65535.0f;
					gs_premultiply_float4(f);
					gs_float4_to_u8x4(dst, f);
					dst += sizeof(*dst) * 4;
					src += sizeof(value);
				}

				src += src_linesize - src_min_line;
			}
		}

		info->format = AV_PIX_FMT_RGBA;
	} else {
		static const enum AVPixelFormat format = AV_PIX_FMT_BGRA;

		bool full_range;
		const enum AVPixelFormat src_format = ffmpeg_image_sws_format(info->format, &full_range);
		sws_ctx = sws_getContext(info->cx, info->cy, src_format, info->cx, info->cy, format, SWS_POINT, NULL,
					 NULL, NULL);
		if (!sws_ctx) {
			blog(LOG_WARNING,
			     "Failed to create scale context "
			     "for '%s'",
			     info->file);
			goto fail;
		}

		if (full_range) {
			/* What swscale sets up for a YUVJ format itself:
			 * full range in, its default (BT.601) matrix. */
			const int *coeff = sws_getCoefficients(SWS_CS_DEFAULT);
			sws_setColorspaceDetails(sws_ctx, coeff, 1, coeff, 0, 0, 1 << 16, 1 << 16);
		}

		uint8_t *pointers[4];
		int linesizes[4];
		ret = av_image_alloc(pointers, linesizes, info->cx, info->cy, format, 32);
		if (ret < 0) {
			blog(LOG_WARNING, "av_image_alloc failed for '%s': %s", info->file, av_err2str(ret));
			sws_freeContext(sws_ctx);
			goto fail;
		}

		ret = sws_scale(sws_ctx, (const uint8_t *const *)frame->data, frame->linesize, 0, info->cy, pointers,
				linesizes);
		sws_freeContext(sws_ctx);

		if (ret < 0) {
			blog(LOG_WARNING, "sws_scale failed for '%s': %s", info->file, av_err2str(ret));
			av_freep(pointers);
			goto fail;
		}

		const size_t linesize = (size_t)info->cx * 4;
		data = bmalloc(info->cy * linesize);
		const uint8_t *src = pointers[0];
		uint8_t *dst = data;
		for (size_t y = 0; y < (size_t)info->cy; y++) {
			memcpy(dst, src, linesize);
			dst += linesize;
			src += linesizes[0];
		}

		av_freep(pointers);

		if (alpha_mode == GS_IMAGE_ALPHA_PREMULTIPLY_SRGB) {
			gs_premultiply_xyza_srgb_loop(data, (size_t)info->cx * info->cy);
		} else if (alpha_mode == GS_IMAGE_ALPHA_PREMULTIPLY) {
			gs_premultiply_xyza_loop(data, (size_t)info->cx * info->cy);
		}

		info->format = format;
	}

	data = ffmpeg_image_orient(info, data, orient);

fail:
	return data;
}

static void *ffmpeg_image_decode(struct ffmpeg_image *info, enum gs_image_alpha_mode alpha_mode)
{
	AVPacket packet = {0};
	void *data = NULL;
	AVFrame *frame = av_frame_alloc();
	int got_frame = 0;
	int ret;

	if (!frame) {
		blog(LOG_WARNING, "Failed to create frame data for '%s'", info->file);
		return NULL;
	}

	ret = av_read_frame(info->fmt_ctx, &packet);
	if (ret < 0) {
		blog(LOG_WARNING, "Failed to read image frame from '%s': %s", info->file, av_err2str(ret));
		goto fail;
	}

	while (!got_frame) {
		ret = avcodec_send_packet(info->decoder_ctx, &packet);
		if (ret == 0)
			ret = avcodec_receive_frame(info->decoder_ctx, frame);

		got_frame = (ret == 0);

		if (ret == AVERROR_EOF || ret == AVERROR(EAGAIN))
			ret = 0;

		if (ret < 0) {
			if (!info->too_large)
				blog(LOG_WARNING, "Failed to decode frame for '%s': %s", info->file, av_err2str(ret));
			goto fail;
		}
	}

	if (ffmpeg_image_too_large(info, frame->width, frame->height))
		goto fail;

	info->cx = frame->width;
	info->cy = frame->height;
	info->format = frame->format;

	data = ffmpeg_image_reformat_frame(info, frame, alpha_mode);

fail:
	av_packet_unref(&packet);
	av_frame_free(&frame);
	return data;
}

void gs_init_image_deps(void) {}

void gs_free_image_deps(void) {}

static inline enum gs_color_format convert_format(enum AVPixelFormat format)
{
	switch (format) {
	case AV_PIX_FMT_RGBA:
		return GS_RGBA;
	case AV_PIX_FMT_BGRA:
		return GS_BGRA;
	case AV_PIX_FMT_BGR0:
		return GS_BGRX;
	case AV_PIX_FMT_RGBA64BE:
		return GS_RGBA16;
	default:
		return GS_BGRX;
	}
}

uint8_t *gs_create_texture_file_data(const char *file, enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out)
{
	struct ffmpeg_image image;
	uint8_t *data = NULL;

	if (ffmpeg_image_init(&image, file)) {
		data = ffmpeg_image_decode(&image, GS_IMAGE_ALPHA_STRAIGHT);
		if (data) {
			*format = convert_format(image.format);
			*cx_out = (uint32_t)image.cx;
			*cy_out = (uint32_t)image.cy;
		}

		ffmpeg_image_free(&image);
	}

	return data;
}

uint8_t *gs_create_texture_file_data2(const char *file, enum gs_image_alpha_mode alpha_mode,
				      enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out)
{
	enum gs_color_space unused;
	return gs_create_texture_file_data3(file, alpha_mode, format, cx_out, cy_out, &unused);
}

uint8_t *gs_create_texture_file_data3(const char *file, enum gs_image_alpha_mode alpha_mode,
				      enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out,
				      enum gs_color_space *space)
{
	struct ffmpeg_image image = {0};
	uint8_t *data = NULL;

	if (ffmpeg_image_init(&image, file)) {
		data = ffmpeg_image_decode(&image, alpha_mode);
		if (data) {
			*format = convert_format(image.format);
			*cx_out = (uint32_t)image.cx;
			*cy_out = (uint32_t)image.cy;
			*space = GS_CS_SRGB;
		}

		ffmpeg_image_free(&image);
	}

	if (data == NULL && !image.too_large)
		data = gs_image_host_fallback_load(file, format, cx_out, cy_out, space);

	return data;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef obs_bswap16
#undef TILE_SIZE
#undef MIN
