#include "build/libobs-media.pch.h"

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

void ffmpeg_image_free(struct ffmpeg_image *info)
{
	avcodec_free_context(&info->decoder_ctx);
	avformat_close_input(&info->fmt_ctx);
}

bool ffmpeg_image_init(struct ffmpeg_image *info, const char *file, uint32_t (*max_size)(void))
{
	int ret;

	if (!file || !*file)
		return false;

	memset(info, 0, sizeof(struct ffmpeg_image));
	info->file = file;
	info->max_size = max_size();

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
