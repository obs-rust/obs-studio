#include "libobs/build/libobs.pch.h"

uint32_t obs_encoder_group_num_started(struct obs_encoder_group *group)
{
	uint32_t num_started;

	pthread_mutex_lock(&group->mutex);
	num_started = group->num_encoders_started;
	pthread_mutex_unlock(&group->mutex);

	return num_started;
}
