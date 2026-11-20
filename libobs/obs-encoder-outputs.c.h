#include "libobs/build/libobs.pch.h"

void obs_encoder_add_output(struct obs_encoder *encoder, struct obs_output *output)
{
	if (!encoder || !output)
		return;

	pthread_mutex_lock(&encoder->outputs_mutex);
	da_push_back(encoder->outputs, &output);
	pthread_mutex_unlock(&encoder->outputs_mutex);
}

void obs_encoder_remove_output(struct obs_encoder *encoder, struct obs_output *output)
{
	if (!encoder || !output)
		return;

	pthread_mutex_lock(&encoder->outputs_mutex);
	da_erase_item(encoder->outputs, &output);
	pthread_mutex_unlock(&encoder->outputs_mutex);
}
