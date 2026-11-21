#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_encoder_group_num_started(). The group's
 * mutex and its num_encoders_started stay in C; Rust decides when each is
 * used: the count is read once, between one lock and one unlock, as every
 * writer (encoder start and stop, set-group) changes it under the same
 * mutex. The group is never NULL here, as in the original. */
void obs_encoder_group_num_started_lock(struct obs_encoder_group *group);
void obs_encoder_group_num_started_lock(struct obs_encoder_group *group)
{
	pthread_mutex_lock(&group->mutex);
}

uint32_t obs_encoder_group_num_started_read(const struct obs_encoder_group *group);
uint32_t obs_encoder_group_num_started_read(const struct obs_encoder_group *group)
{
	return group->num_encoders_started;
}

void obs_encoder_group_num_started_unlock(struct obs_encoder_group *group);
void obs_encoder_group_num_started_unlock(struct obs_encoder_group *group)
{
	pthread_mutex_unlock(&group->mutex);
}
