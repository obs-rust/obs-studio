#include "libobs/build/libobs.pch.h"

void obs_encoder_group_actually_destroy(obs_encoder_group_t *group);

obs_encoder_group_t *obs_encoder_group_create(void)
{
	struct obs_encoder_group *group = bzalloc(sizeof(struct obs_encoder_group));

	pthread_mutex_init_value(&group->mutex);
	if (pthread_mutex_init(&group->mutex, NULL) != 0) {
		bfree(group);
		return NULL;
	}

	return group;
}

void obs_encoder_group_actually_destroy(obs_encoder_group_t *group)
{
	for (size_t i = 0; i < group->encoders.num; i++) {
		struct obs_encoder *encoder = group->encoders.array[i];
		encoder->encoder_group = NULL;
		obs_encoder_release(encoder);
	}

	da_free(group->encoders);
	pthread_mutex_unlock(&group->mutex);
	pthread_mutex_destroy(&group->mutex);

	bfree(group);
}

void obs_encoder_group_destroy(obs_encoder_group_t *group)
{
	if (!group)
		return;

	pthread_mutex_lock(&group->mutex);

	if (group->num_encoders_started) {
		group->destroy_on_stop = true;
		pthread_mutex_unlock(&group->mutex);
		return;
	}

	obs_encoder_group_actually_destroy(group);
}
