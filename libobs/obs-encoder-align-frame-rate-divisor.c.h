#include "libobs/build/libobs.pch.h"

#ifdef ENCODER_ALIGN_SOURCE
/* The unchanged source characterization supplies its local registration. */
static void receive_video(void *param, struct video_data *frame);
#else
/* The C reference shares the production donor's callback, across unity groups. */
void (*obs_encoder_align_receive_video(void))(void *, struct video_data *);
#define receive_video (*obs_encoder_align_receive_video())
#endif

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

#undef receive_video
