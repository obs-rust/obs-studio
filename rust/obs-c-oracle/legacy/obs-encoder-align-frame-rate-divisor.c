#include "libobs/build/libobs.pch.h"

static void receive_video(void *param, struct video_data *frame);

/* A grouped encoder starts only on the group's start frame, but video-io
 * decides which frames it delivers with a per-input counter that starts at 0
 * on connect. A frame already queued before the group start consumes that
 * first slot, so an encoder with a frame-rate divisor above 1 is one phase
 * ahead and never sees the start frame. Telling video-io where the phase
 * starts makes the group's start frame the first one it delivers. */
void obs_encoder_align_frame_rate_divisor(obs_encoder_t *encoder, uint64_t start_timestamp)
{
	if (!encoder || !encoder->media)
		return;
	if (encoder->frame_rate_divisor < 2)
		return;

	video_output_align_frame_rate_divisor(encoder->media, start_timestamp, receive_video, encoder);
}
