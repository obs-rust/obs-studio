#include "libobs/build/libobs.pch.h"

void obs_encoder_audio_buffers_free(struct obs_encoder_audio_buffers_view *view)
{
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		deque_free(&view->input[i]);
		bfree(view->output[i]);
		view->output[i] = NULL;
	}
}

void obs_encoder_audio_buffers_reset(struct obs_encoder_audio_buffers_view *view)
{
	obs_encoder_audio_buffers_free(view);
	for (size_t i = 0; i < *view->planes; i++)
		view->output[i] = bmalloc(*view->bytes);
}

void obs_encoder_audio_buffers_initialize(struct obs_encoder_audio_buffers_view *view, uint32_t rate,
					  enum audio_format format, enum speaker_layout speakers)
{
	*view->rate = rate;
	*view->planes = get_audio_planes(format, speakers);
	*view->blocksize = get_audio_size(format, speakers, 1);
	*view->frames = view->frame_size(view->context);
	*view->bytes = *view->blocksize * *view->frames;
	obs_encoder_audio_buffers_reset(view);
}
