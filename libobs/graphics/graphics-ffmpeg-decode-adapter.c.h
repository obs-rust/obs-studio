#include "libobs/build/libobs-image-decode-adapter.pch.h"

/* Private FFmpeg ownership. No decoder decisions or retry loop live here. */
struct gs_image_decode_session {
	struct gs_image_decode_context *context;
	AVPacket packet;
	AVFrame *frame;
	int alpha_mode;
};

void *gs_image_decode_allocate(void *opaque, int alpha_mode);
int gs_image_decode_read(void *opaque);
int gs_image_decode_send(void *opaque);
int gs_image_decode_receive(void *opaque);
bool gs_image_decode_rejected(void *opaque);
bool gs_image_decode_check_size(void *opaque);
void gs_image_decode_update(void *opaque);
void *gs_image_decode_reformat(void *opaque);
void gs_image_decode_warn(void *opaque, int kind, int error);
void gs_image_decode_cleanup(void *opaque);

void *gs_image_decode_allocate(void *opaque, int alpha_mode)
{
	AVFrame *frame = av_frame_alloc();
	if (!frame)
		return NULL;
	struct gs_image_decode_session *session = bzalloc(sizeof(*session));
	session->context = opaque;
	session->frame = frame;
	session->alpha_mode = alpha_mode;
	return session;
}

int gs_image_decode_read(void *opaque)
{
	struct gs_image_decode_session *session = opaque;
	return av_read_frame(session->context->image->fmt_ctx, &session->packet);
}

static int gs_image_decode_status(int result)
{
	return (result == AVERROR_EOF || result == AVERROR(EAGAIN)) ? 1 : result;
}

int gs_image_decode_send(void *opaque)
{
	struct gs_image_decode_session *session = opaque;
	return gs_image_decode_status(avcodec_send_packet(session->context->image->decoder_ctx, &session->packet));
}

int gs_image_decode_receive(void *opaque)
{
	struct gs_image_decode_session *session = opaque;
	return gs_image_decode_status(avcodec_receive_frame(session->context->image->decoder_ctx, session->frame));
}

bool gs_image_decode_rejected(void *opaque)
{
	struct gs_image_decode_session *session = opaque;
	return session->context->image->too_large;
}

bool gs_image_decode_check_size(void *opaque)
{
	struct gs_image_decode_session *session = opaque;
	return session->context->check_size(session->context->image, session->frame->width, session->frame->height);
}

void gs_image_decode_update(void *opaque)
{
	struct gs_image_decode_session *session = opaque;
	session->context->image->cx = session->frame->width;
	session->context->image->cy = session->frame->height;
	session->context->image->format = session->frame->format;
}

void *gs_image_decode_reformat(void *opaque)
{
	struct gs_image_decode_session *session = opaque;
	return session->context->convert(session->context->image, session->frame,
					 (enum gs_image_alpha_mode)session->alpha_mode);
}

void gs_image_decode_warn(void *opaque, int kind, int error)
{
	struct gs_image_decode_context *context = opaque;
	struct ffmpeg_image *info = context->image;
	switch (kind) {
	case 0:
		blog(LOG_WARNING, "Failed to create frame data for '%s'", info->file);
		break;
	case 1:
		blog(LOG_WARNING, "Failed to read image frame from '%s': %s", info->file, av_err2str(error));
		break;
	case 2:
		blog(LOG_WARNING, "Failed to decode frame for '%s': %s", info->file, av_err2str(error));
		break;
	}
}

void gs_image_decode_cleanup(void *opaque)
{
	struct gs_image_decode_session *session = opaque;
	av_packet_unref(&session->packet);
	av_frame_free(&session->frame);
	bfree(session);
}
