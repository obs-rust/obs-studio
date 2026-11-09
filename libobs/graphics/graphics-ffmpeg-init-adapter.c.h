#include "build/libobs-media.pch.h"

/* A picture wider or taller than the graphics device's texture size limit
 * cannot become a texture. Say so once, before anything is decoded. */
bool ffmpeg_image_too_large(struct ffmpeg_image *info, int cx, int cy)
{
	if (!gs_ffmpeg_image_too_large(info->max_size, cx, cy))
		return false;

	if (!info->too_large)
		blog(LOG_WARNING,
		     "Image '%s' is %dx%d, larger than the maximum texture size of "
		     "the graphics device (%" PRIu32 "x%" PRIu32 "); not loading it",
		     info->file, cx, cy, info->max_size, info->max_size);
	info->too_large = true;
	return true;
}

/* Private FFmpeg field/call adapter. Operation values match image_init::Operation.
 * All lifecycle decisions and failure cleanup sequencing are in safe Rust. */
/* FFmpeg can invoke get_buffer2 on a worker context distinct from the
 * owning decoder context. Keep the exact callback context and frame together. */
struct ffmpeg_image_adapter_buffer {
	AVCodecContext *ctx;
	AVFrame *frame;
};

static int ffmpeg_image_adapter_get_buffer(AVCodecContext *ctx, AVFrame *frame, int flags)
{
	struct ffmpeg_image *info = ctx->opaque;
	struct ffmpeg_image_adapter_buffer buffer = {ctx, frame};
	return info->get_buffer(info, &buffer, flags);
}

int obs_ffmpeg_image_operation(struct ffmpeg_image *info, uint32_t op, int value, void *extra,
			       int (*get_buffer)(struct ffmpeg_image *, void *, int), uint32_t (*max_size)(void))
{
	switch (op) {
	case 0: /* Reset */
		memset(info, 0, sizeof(*info));
		info->file = extra;
		info->max_size = max_size();
		return 0;
	case 1:
		return avformat_open_input(&info->fmt_ctx, info->file, NULL, NULL);
	case 2:
		return (int)info->fmt_ctx->nb_streams;
	case 3:
		return info->fmt_ctx->streams[0]->codecpar->codec_type == AVMEDIA_TYPE_VIDEO;
	case 4:
		return info->fmt_ctx->streams[0]->codecpar->codec_id != AV_CODEC_ID_NONE;
	case 5:
		return avformat_find_stream_info(info->fmt_ctx, NULL);
	case 6:
		return av_find_best_stream(info->fmt_ctx, AVMEDIA_TYPE_VIDEO, -1, 1, NULL, 0);
	case 7:
		info->init_decoder = avcodec_find_decoder(info->fmt_ctx->streams[value]->codecpar->codec_id);
		return info->init_decoder != NULL;
	case 8: {
		const AVCodecParameters *par = info->fmt_ctx->streams[value]->codecpar;
		return ffmpeg_image_too_large(info, par->width, par->height);
	}
	case 9:
		info->decoder_ctx = avcodec_alloc_context3(info->init_decoder);
		return 0;
	case 10:
		avcodec_parameters_to_context(info->decoder_ctx, info->fmt_ctx->streams[value]->codecpar);
		info->decoder_ctx->opaque = info;
		info->get_buffer = get_buffer;
		info->decoder_ctx->get_buffer2 = ffmpeg_image_adapter_get_buffer;
		return 0;
	case 11:
		return avcodec_open2(info->decoder_ctx, info->init_decoder, NULL);
	case 12:
		avcodec_free_context(&info->decoder_ctx);
		return 0;
	case 13:
		avformat_close_input(&info->fmt_ctx);
		return 0;
	case 14:
		blog(LOG_WARNING, "Failed to open file '%s': %s", info->file, av_err2str(value));
		return 0;
	case 15:
		blog(LOG_WARNING, "Could not find stream info for file '%s': %s", info->file, av_err2str(value));
		return 0;
	case 16:
		blog(LOG_WARNING, "Couldn't find video stream in file '%s': %s", info->file, av_err2str(value));
		return 0;
	case 17:
		blog(LOG_WARNING, "Failed to find decoder for file '%s'", info->file);
		return 0;
	case 18:
		blog(LOG_WARNING, "Failed to open video codec for file '%s': %s", info->file, av_err2str(value));
		return 0;
	case 19: {
		const struct ffmpeg_image_adapter_buffer *buffer = extra;
		return ffmpeg_image_too_large(info, buffer->ctx->width, buffer->ctx->height) ? AVERROR(EINVAL) : 0;
	}
	case 20: {
		const struct ffmpeg_image_adapter_buffer *buffer = extra;
		return avcodec_default_get_buffer2(buffer->ctx, buffer->frame, value);
	}
	default:
		return AVERROR(EINVAL);
	}
}
