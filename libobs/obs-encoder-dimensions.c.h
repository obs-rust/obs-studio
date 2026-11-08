#include "libobs/build/libobs.pch.h"

uint32_t obs_encoder_get_width(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_width"))
		return 0;

	switch (obs_encoder_size_video_route((int)encoder->info.type, encoder->media != NULL, encoder->scaled_width)) {
	case OBS_ENCODER_SIZE_WRONG_TYPE:
		blog(LOG_WARNING,
		     "obs_encoder_get_width: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return 0;
	case OBS_ENCODER_SIZE_STORED:
		return encoder->scaled_width;
	case OBS_ENCODER_SIZE_MEDIA:
		return video_output_get_width(encoder->media);
	case OBS_ENCODER_SIZE_NO_MEDIA:
	default:
		return 0;
	}
}

uint32_t obs_encoder_get_height(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_height"))
		return 0;

	switch (obs_encoder_size_video_route((int)encoder->info.type, encoder->media != NULL, encoder->scaled_height)) {
	case OBS_ENCODER_SIZE_WRONG_TYPE:
		blog(LOG_WARNING,
		     "obs_encoder_get_height: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return 0;
	case OBS_ENCODER_SIZE_STORED:
		return encoder->scaled_height;
	case OBS_ENCODER_SIZE_MEDIA:
		return video_output_get_height(encoder->media);
	case OBS_ENCODER_SIZE_NO_MEDIA:
	default:
		return 0;
	}
}
