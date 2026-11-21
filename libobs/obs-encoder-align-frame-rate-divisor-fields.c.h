#include "libobs/build/libobs.pch.h"

void *obs_encoder_align_media(const obs_encoder_t *encoder);
uint32_t obs_encoder_align_divisor(const obs_encoder_t *encoder);
void obs_encoder_align_forward(video_t *media, uint64_t timestamp, void (*callback)(void *, struct video_data *),
			       obs_encoder_t *encoder);

void *obs_encoder_align_media(const obs_encoder_t *encoder)
{
	return encoder->media;
}

uint32_t obs_encoder_align_divisor(const obs_encoder_t *encoder)
{
	return encoder->frame_rate_divisor;
}

void obs_encoder_align_forward(video_t *media, uint64_t timestamp, void (*callback)(void *, struct video_data *),
			       obs_encoder_t *encoder)
{
	video_output_align_frame_rate_divisor(media, timestamp, callback, encoder);
}
