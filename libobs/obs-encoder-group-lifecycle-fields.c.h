#include "libobs/build/libobs.pch.h"

/* Native primitives only; Rust owns the lifecycle ordering. */
struct obs_encoder_group_lifecycle_fields obs_encoder_group_lifecycle_fields(obs_encoder_group_t *group)
{
	return (struct obs_encoder_group_lifecycle_fields){&group->num_encoders_started, &group->destroy_on_stop};
}

obs_encoder_group_t *obs_encoder_group_lifecycle_alloc(void)
{
	return bzalloc(sizeof(struct obs_encoder_group));
}

void obs_encoder_group_lifecycle_free(obs_encoder_group_t *group)
{
	bfree(group);
}

bool obs_encoder_group_lifecycle_init(obs_encoder_group_t *group)
{
	pthread_mutex_init_value(&group->mutex);
	return pthread_mutex_init(&group->mutex, NULL) == 0;
}

void obs_encoder_group_lifecycle_lock(obs_encoder_group_t *group)
{
	pthread_mutex_lock(&group->mutex);
}

void obs_encoder_group_lifecycle_unlock(obs_encoder_group_t *group)
{
	pthread_mutex_unlock(&group->mutex);
}

void obs_encoder_group_lifecycle_mutex_destroy(obs_encoder_group_t *group)
{
	pthread_mutex_destroy(&group->mutex);
}

size_t obs_encoder_group_lifecycle_count(obs_encoder_group_t *group)
{
	return group->encoders.num;
}

obs_encoder_t *obs_encoder_group_lifecycle_encoder(obs_encoder_group_t *group, size_t index)
{
	return group->encoders.array[index];
}

void obs_encoder_group_lifecycle_clear(obs_encoder_t *encoder)
{
	encoder->encoder_group = NULL;
}

void obs_encoder_group_lifecycle_free_list(obs_encoder_group_t *group)
{
	da_free(group->encoders);
}
