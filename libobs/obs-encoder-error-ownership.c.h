#include "libobs/build/libobs.pch.h"

void obs_encoder_set_last_error(obs_encoder_t *encoder, const char *message)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_set_last_error"))
		return;

	pthread_mutex_lock(&encoder->last_error_mutex);
	if (encoder->last_error_message)
		bfree(encoder->last_error_message);

	if (message)
		encoder->last_error_message = bstrdup(message);
	else
		encoder->last_error_message = NULL;
	pthread_mutex_unlock(&encoder->last_error_mutex);
}

char *obs_encoder_dup_last_error(obs_encoder_t *encoder)
{
	pthread_mutex_lock(&encoder->last_error_mutex);
	char *message = bstrdup(encoder->last_error_message);
	pthread_mutex_unlock(&encoder->last_error_mutex);
	return message;
}
