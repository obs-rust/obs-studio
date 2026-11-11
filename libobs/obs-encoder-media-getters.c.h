#include "libobs/build/libobs.pch.h"

video_t *obs_encoder_video(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_video"))
		return NULL;
	if (obs_encoder_audio_start_not_video((int)encoder->info.type)) {
		blog(LOG_WARNING,
		     "obs_encoder_set_video: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return NULL;
	}

	return encoder->fps_override ? encoder->fps_override : encoder->media;
}

video_t *obs_encoder_parent_video(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_parent_video"))
		return NULL;
	if (obs_encoder_audio_start_not_video((int)encoder->info.type)) {
		blog(LOG_WARNING,
		     "obs_encoder_parent_video: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return NULL;
	}

	return encoder->media;
}

audio_t *obs_encoder_audio(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_audio"))
		return NULL;
	if (obs_encoder_bind_not_audio((int)encoder->info.type)) {
		blog(LOG_WARNING,
		     "obs_encoder_set_audio: "
		     "encoder '%s' is not an audio encoder",
		     obs_encoder_get_name(encoder));
		return NULL;
	}

	return encoder->media;
}
