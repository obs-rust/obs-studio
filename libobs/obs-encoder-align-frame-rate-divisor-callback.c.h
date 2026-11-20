#include "libobs/build/libobs.pch.h"

/* The definition is in the encoder donor in this same unity group. Never
 * replace it: video-io matches the address registered by start_raw_video. */
static void receive_video(void *param, struct video_data *frame);

void obs_encoder_align_frame_rate_divisor_with_callback(obs_encoder_t *encoder, uint64_t timestamp,
							void (*callback)(void *, struct video_data *));

void obs_encoder_align_frame_rate_divisor(obs_encoder_t *encoder, uint64_t timestamp)
{
	obs_encoder_align_frame_rate_divisor_with_callback(encoder, timestamp, receive_video);
}
