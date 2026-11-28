#include "libobs/build/libobs.pch.h"

#define encoder_active(encoder) os_atomic_load_bool(&encoder->active)

void obs_encoder_set_audio(obs_encoder_t *encoder, audio_t *audio)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_set_audio"))
		return;
	if (obs_encoder_bind_not_audio((int)encoder->info.type)) {
		blog(LOG_WARNING,
		     "obs_encoder_set_audio: "
		     "encoder '%s' is not an audio encoder",
		     obs_encoder_get_name(encoder));
		return;
	}
	if (encoder_active(encoder)) {
		blog(LOG_WARNING,
		     "encoder '%s': Cannot apply a new audio_t "
		     "object while the encoder is active",
		     obs_encoder_get_name(encoder));
		return;
	}

	struct obs_encoder_bind_timebase timebase;

	if (audio) {
		encoder->media = audio;
		timebase = obs_encoder_bind_audio_timebase(true, audio_output_get_sample_rate(audio));
		encoder->timebase_num = timebase.num;
		encoder->timebase_den = timebase.den;
	} else {
		encoder->media = NULL;
		timebase = obs_encoder_bind_audio_timebase(false, 0);
		encoder->timebase_num = timebase.num;
		encoder->timebase_den = timebase.den;
	}
}

#undef encoder_active
