#include "libobs/build/libobs.pch.h"

void obs_encoder_clear_roi(obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_clear_roi"))
		return;
	if (!encoder->roi.num)
		return;
	pthread_mutex_lock(&encoder->roi_mutex);
	da_clear(encoder->roi);
	encoder->roi_increment++;
	pthread_mutex_unlock(&encoder->roi_mutex);
}
