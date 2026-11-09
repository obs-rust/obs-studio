#include "libobs/build/libobs.pch.h"

#define encoder_active(encoder) os_atomic_load_bool(&encoder->active)

void obs_encoder_set_scaled_size(obs_encoder_t *encoder, uint32_t width, uint32_t height)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_set_scaled_size"))
		return;

	switch (obs_encoder_size_scaled_gate((int)encoder->info.type, encoder_active(encoder), encoder->initialized)) {
	case OBS_ENCODER_SCALED_NOT_VIDEO:
		blog(LOG_WARNING,
		     "obs_encoder_set_scaled_size: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return;
	case OBS_ENCODER_SCALED_ACTIVE:
		blog(LOG_WARNING,
		     "encoder '%s': Cannot set the scaled "
		     "resolution while the encoder is active",
		     obs_encoder_get_name(encoder));
		return;
	case OBS_ENCODER_SCALED_INITIALIZED:
		blog(LOG_WARNING,
		     "encoder '%s': Cannot set the scaled resolution "
		     "after the encoder has been initialized",
		     obs_encoder_get_name(encoder));
		return;
	case OBS_ENCODER_SCALED_CHECK:
	default:
		break;
	}

	const struct video_output_info *voi;
	voi = video_output_get_info(encoder->media);
	if (obs_encoder_size_scaled_match(voi != NULL, voi ? voi->width : 0, voi ? voi->height : 0, width, height) ==
	    OBS_ENCODER_SCALED_DISABLE) {
		blog(LOG_WARNING,
		     "encoder '%s': Scaled resolution "
		     "matches output resolution, scaling "
		     "disabled",
		     obs_encoder_get_name(encoder));
		encoder->scaled_width = encoder->scaled_height = 0;
		return;
	}

	encoder->scaled_width = width;
	encoder->scaled_height = height;
}

void obs_encoder_set_gpu_scale_type(obs_encoder_t *encoder, enum obs_scale_type gpu_scale_type)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_set_gpu_scale_type"))
		return;
	if (obs_encoder_audio_start_not_video((int)encoder->info.type)) {
		blog(LOG_WARNING,
		     "obs_encoder_set_gpu_scale_type: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return;
	}
	if (encoder_active(encoder)) {
		blog(LOG_WARNING,
		     "encoder '%s': Cannot enable GPU scaling "
		     "while the encoder is active",
		     obs_encoder_get_name(encoder));
		return;
	}
	if (encoder->initialized) {
		blog(LOG_WARNING,
		     "encoder '%s': Cannot enable GPU scaling "
		     "after the encoder has been initialized",
		     obs_encoder_get_name(encoder));
		return;
	}

	encoder->gpu_scale_type = gpu_scale_type;
}

bool obs_encoder_set_frame_rate_divisor(obs_encoder_t *encoder, uint32_t frame_rate_divisor)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_set_frame_rate_divisor"))
		return false;

	if (obs_encoder_audio_start_not_video((int)encoder->info.type)) {
		blog(LOG_WARNING,
		     "obs_encoder_set_frame_rate_divisor: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return false;
	}

	if (encoder_active(encoder)) {
		blog(LOG_WARNING,
		     "encoder '%s': Cannot set frame rate divisor "
		     "while the encoder is active",
		     obs_encoder_get_name(encoder));
		return false;
	}

	if (encoder->initialized) {
		blog(LOG_WARNING,
		     "encoder '%s': Cannot set frame rate divisor "
		     "after the encoder has been initialized",
		     obs_encoder_get_name(encoder));
		return false;
	}

	if (!obs_encoder_audio_start_divisor_ok(frame_rate_divisor)) {
		blog(LOG_WARNING,
		     "encoder '%s': Cannot set frame "
		     "rate divisor to 0",
		     obs_encoder_get_name(encoder));
		return false;
	}

	encoder->frame_rate_divisor = frame_rate_divisor;

	if (encoder->fps_override) {
		video_output_free_frame_rate_divisor(encoder->fps_override);
		encoder->fps_override = NULL;
	}

	if (encoder->media) {
		encoder->fps_override =
			video_output_create_with_frame_rate_divisor(encoder->media, encoder->frame_rate_divisor);
	}

	return true;
}

#undef encoder_active
