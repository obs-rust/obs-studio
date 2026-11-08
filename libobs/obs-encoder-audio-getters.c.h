#include "libobs/build/libobs.pch.h"

uint32_t obs_encoder_get_sample_rate(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_sample_rate"))
		return 0;

	switch (obs_encoder_size_rate_route((int)encoder->info.type, encoder->media != NULL, encoder->samplerate)) {
	case OBS_ENCODER_SIZE_WRONG_TYPE:
		blog(LOG_WARNING,
		     "obs_encoder_get_sample_rate: "
		     "encoder '%s' is not an audio encoder",
		     obs_encoder_get_name(encoder));
		return 0;
	case OBS_ENCODER_SIZE_STORED:
		return encoder->samplerate;
	case OBS_ENCODER_SIZE_MEDIA:
		return audio_output_get_sample_rate(encoder->media);
	case OBS_ENCODER_SIZE_NO_MEDIA:
	default:
		return 0;
	}
}

size_t obs_encoder_get_frame_size(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_frame_size"))
		return 0;
	if (encoder->info.type != OBS_ENCODER_AUDIO) {
		blog(LOG_WARNING,
		     "obs_encoder_get_frame_size: "
		     "encoder '%s' is not an audio encoder",
		     obs_encoder_get_name(encoder));
		return 0;
	}

	return encoder->framesize;
}

size_t obs_encoder_get_mixer_index(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_mixer_index"))
		return 0;

	if (encoder->info.type != OBS_ENCODER_AUDIO) {
		blog(LOG_WARNING,
		     "obs_encoder_get_mixer_index: "
		     "encoder '%s' is not an audio encoder",
		     obs_encoder_get_name(encoder));
		return 0;
	}

	return encoder->mixer_idx;
}
