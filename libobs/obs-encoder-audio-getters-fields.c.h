#include "libobs/build/libobs.pch.h"

struct obs_encoder_audio_getters_fields obs_encoder_audio_getters_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_audio_getters_fields){
		encoder ? &encoder->info.type : NULL,  encoder ? &encoder->media : NULL,
		encoder ? &encoder->samplerate : NULL, encoder ? &encoder->framesize : NULL,
		encoder ? &encoder->mixer_idx : NULL,  encoder ? &encoder->context.name : NULL};
}
