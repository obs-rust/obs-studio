#include "libobs/build/libobs-media.pch.h"

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
