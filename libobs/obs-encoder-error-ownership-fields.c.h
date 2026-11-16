#include "libobs/build/libobs.pch.h"

char **obs_encoder_error_ownership_lock(obs_encoder_t *encoder);
void obs_encoder_error_ownership_unlock(obs_encoder_t *encoder);

char **obs_encoder_error_ownership_lock(obs_encoder_t *encoder)
{
	pthread_mutex_lock(&encoder->last_error_mutex);
	return &encoder->last_error_message;
}

void obs_encoder_error_ownership_unlock(obs_encoder_t *encoder)
{
	pthread_mutex_unlock(&encoder->last_error_mutex);
}
