#include "build/libobs-media.pch.h"

static void *ffmpeg_image_copy_data_straight(struct ffmpeg_image *info, AVFrame *frame)
{
	const size_t linesize = (size_t)info->cx * 4;
	const size_t totalsize = (size_t)info->cy * linesize;
	void *data = bmalloc(totalsize);

	gs_ffmpeg_image_copy_rows(data, frame->data[0], (size_t)info->cx, (size_t)info->cy, (size_t)frame->linesize[0]);

	return data;
}

static void *ffmpeg_image_orient(struct ffmpeg_image *info, void *in_data, int orient)
{
	const size_t sx = (size_t)info->cx;
	const size_t sy = (size_t)info->cy;
	uint8_t *data = NULL;

	if (!gs_ffmpeg_image_orient_needed(orient))
		return in_data;

	data = bmalloc(sx * 4 * sy);

	if (gs_ffmpeg_image_orient_swaps(orient)) {
		info->cx = (int)sy;
		info->cy = (int)sx;
	}

	gs_ffmpeg_image_orient_copy(data, in_data, sx, sy, orient);

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
static int display_matrix_to_orientation(const int32_t *matrix)
{
	return gs_ffmpeg_image_matrix_orientation(matrix[0], matrix[1], matrix[3], matrix[4]);
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

/* graphics-ffmpeg-route.c gives the pixel formats, the texture formats and
 * the alpha modes as their values. */
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUV420P == AV_PIX_FMT_YUV420P, "AV_PIX_FMT_YUV420P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUV422P == AV_PIX_FMT_YUV422P, "AV_PIX_FMT_YUV422P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUV444P == AV_PIX_FMT_YUV444P, "AV_PIX_FMT_YUV444P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUV411P == AV_PIX_FMT_YUV411P, "AV_PIX_FMT_YUV411P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ420P == AV_PIX_FMT_YUVJ420P, "AV_PIX_FMT_YUVJ420P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ422P == AV_PIX_FMT_YUVJ422P, "AV_PIX_FMT_YUVJ422P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ444P == AV_PIX_FMT_YUVJ444P, "AV_PIX_FMT_YUVJ444P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_RGBA == AV_PIX_FMT_RGBA, "AV_PIX_FMT_RGBA");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_BGRA == AV_PIX_FMT_BGRA, "AV_PIX_FMT_BGRA");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUV440P == AV_PIX_FMT_YUV440P, "AV_PIX_FMT_YUV440P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ440P == AV_PIX_FMT_YUVJ440P, "AV_PIX_FMT_YUVJ440P");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_RGBA64BE == AV_PIX_FMT_RGBA64BE, "AV_PIX_FMT_RGBA64BE");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_BGR0 == AV_PIX_FMT_BGR0, "AV_PIX_FMT_BGR0");
_Static_assert(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ411P == AV_PIX_FMT_YUVJ411P, "AV_PIX_FMT_YUVJ411P");
_Static_assert(GS_FFMPEG_IMAGE_GS_RGBA == GS_RGBA, "GS_RGBA");
_Static_assert(GS_FFMPEG_IMAGE_GS_BGRX == GS_BGRX, "GS_BGRX");
_Static_assert(GS_FFMPEG_IMAGE_GS_BGRA == GS_BGRA, "GS_BGRA");
_Static_assert(GS_FFMPEG_IMAGE_GS_RGBA16 == GS_RGBA16, "GS_RGBA16");
_Static_assert(GS_FFMPEG_IMAGE_ALPHA_STRAIGHT == GS_IMAGE_ALPHA_STRAIGHT, "GS_IMAGE_ALPHA_STRAIGHT");
_Static_assert(GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY_SRGB == GS_IMAGE_ALPHA_PREMULTIPLY_SRGB,
	       "GS_IMAGE_ALPHA_PREMULTIPLY_SRGB");
_Static_assert(GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY == GS_IMAGE_ALPHA_PREMULTIPLY, "GS_IMAGE_ALPHA_PREMULTIPLY");

/* graphics-ffmpeg-rgba64.c gives the alpha modes as their values. */
_Static_assert(GS_FFMPEG_RGBA64_ALPHA_STRAIGHT == GS_IMAGE_ALPHA_STRAIGHT, "GS_IMAGE_ALPHA_STRAIGHT");
_Static_assert(GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB == GS_IMAGE_ALPHA_PREMULTIPLY_SRGB,
	       "GS_IMAGE_ALPHA_PREMULTIPLY_SRGB");
_Static_assert(GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY == GS_IMAGE_ALPHA_PREMULTIPLY, "GS_IMAGE_ALPHA_PREMULTIPLY");

void *ffmpeg_image_reformat_frame(struct ffmpeg_image *info, AVFrame *frame, enum gs_image_alpha_mode alpha_mode)
{
	struct SwsContext *sws_ctx = NULL;
	void *data = NULL;
	int ret = 0;

	const int orient = ffmpeg_image_get_orientation(frame);
	const struct gs_ffmpeg_image_plan plan = gs_ffmpeg_image_plan(info->format, (int)alpha_mode);

	switch (plan.route) {
	case GS_FFMPEG_IMAGE_ROUTE_COPY:
		data = ffmpeg_image_copy_data_straight(info, frame);
		break;
	case GS_FFMPEG_IMAGE_ROUTE_PREMULTIPLY_ROWS: {
		const size_t linesize = (size_t)info->cx * 4;
		const size_t totalsize = (size_t)info->cy * linesize;
		data = bmalloc(totalsize);
		const size_t src_linesize = (size_t)frame->linesize[0];
		const size_t min_line = linesize < src_linesize ? linesize : src_linesize;
		uint8_t *dst = data;
		const uint8_t *src = frame->data[0];
		const size_t row_elements = min_line >> 2;
		if (plan.premultiply == GS_FFMPEG_IMAGE_PREMULTIPLY_SRGB) {
			for (int y = 0; y < info->cy; y++) {
				gs_premultiply_xyza_srgb_loop_restrict(dst, src, row_elements);
				dst += linesize;
				src += src_linesize;
			}
		} else if (plan.premultiply == GS_FFMPEG_IMAGE_PREMULTIPLY_LINEAR) {
			for (int y = 0; y < info->cy; y++) {
				gs_premultiply_xyza_loop_restrict(dst, src, row_elements);
				dst += linesize;
				src += src_linesize;
			}
		}
		break;
	}
	case GS_FFMPEG_IMAGE_ROUTE_RGBA64: {
		const size_t dst_linesize = (size_t)info->cx * 4;
		data = bmalloc((size_t)info->cy * dst_linesize);
		gs_ffmpeg_image_rgba64be_rows(data, frame->data[0], info->cx, info->cy, (size_t)frame->linesize[0],
					      (int)alpha_mode);

		info->format = (enum AVPixelFormat)plan.out_format;
		break;
	}
	default: {
		/* GS_FFMPEG_IMAGE_ROUTE_SWS */
		static const enum AVPixelFormat format = AV_PIX_FMT_BGRA;

		sws_ctx = sws_getContext(info->cx, info->cy, (enum AVPixelFormat)plan.sws_format, info->cx, info->cy,
					 format, SWS_POINT, NULL, NULL, NULL);
		if (!sws_ctx) {
			blog(LOG_WARNING,
			     "Failed to create scale context "
			     "for '%s'",
			     info->file);
			goto fail;
		}

		if (plan.full_range) {
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
		data = bmalloc((size_t)info->cy * linesize);
		const uint8_t *src = pointers[0];
		uint8_t *dst = data;
		for (size_t y = 0; y < (size_t)info->cy; y++) {
			memcpy(dst, src, linesize);
			dst += linesize;
			src += linesizes[0];
		}

		av_freep(pointers);

		if (plan.premultiply == GS_FFMPEG_IMAGE_PREMULTIPLY_SRGB) {
			gs_premultiply_xyza_srgb_loop(data, (size_t)info->cx * (size_t)info->cy);
		} else if (plan.premultiply == GS_FFMPEG_IMAGE_PREMULTIPLY_LINEAR) {
			gs_premultiply_xyza_loop(data, (size_t)info->cx * (size_t)info->cy);
		}

		info->format = (enum AVPixelFormat)plan.out_format;
		break;
	}
	}

	data = ffmpeg_image_orient(info, data, orient);

fail:
	return data;
}

void gs_init_image_deps(void);
void gs_free_image_deps(void);

void gs_init_image_deps(void) {}

void gs_free_image_deps(void) {}

uint8_t *gs_create_texture_file_data(const char *file, enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out)
{
	struct ffmpeg_image image;
	uint8_t *data = NULL;

	if (ffmpeg_image_init(&image, file, gs_image_max_texture_size)) {
		struct gs_image_decode_context decode = {&image, ffmpeg_image_too_large, ffmpeg_image_reformat_frame};
		data = gs_ffmpeg_image_decode(&decode, (int)GS_IMAGE_ALPHA_STRAIGHT);
		if (data) {
			*format = (enum gs_color_format)gs_ffmpeg_image_color_format(image.format);
			*cx_out = (uint32_t)image.cx;
			*cy_out = (uint32_t)image.cy;
		}

		ffmpeg_image_free(&image);
	}

	return data;
}

uint8_t *gs_create_texture_file_data3(const char *file, enum gs_image_alpha_mode alpha_mode,
				      enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out,
				      enum gs_color_space *space)
{
	struct ffmpeg_image image = {0};
	uint8_t *data = NULL;

	if (ffmpeg_image_init(&image, file, gs_image_max_texture_size)) {
		struct gs_image_decode_context decode = {&image, ffmpeg_image_too_large, ffmpeg_image_reformat_frame};
		data = gs_ffmpeg_image_decode(&decode, (int)alpha_mode);
		if (data) {
			*format = (enum gs_color_format)gs_ffmpeg_image_color_format(image.format);
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
