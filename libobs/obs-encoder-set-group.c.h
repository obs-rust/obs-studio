#include "libobs/build/libobs.pch.h"

bool obs_encoder_set_group(obs_encoder_t *encoder, obs_encoder_group_t *group)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_set_group"))
		return false;

	if (obs_encoder_active(encoder)) {
		blog(LOG_ERROR, "obs_encoder_set_group: encoder '%s' is already active", obs_encoder_get_name(encoder));
		return false;
	}

	/* Already in this group: nothing to leave or join. */
	if (group && group == encoder->encoder_group)
		return true;

	/* Check the new group before leaving the old one, so a refused move
	 * leaves the encoder where it was (#2932). The join below checks
	 * again under the new group's mutex; the two group mutexes are never
	 * held together. */
	if (group && obs_encoder_group_num_started(group)) {
		blog(LOG_ERROR, "obs_encoder_set_group: specified group has started encoders");
		return false;
	}

	if (encoder->encoder_group) {
		struct obs_encoder_group *old_group = encoder->encoder_group;
		pthread_mutex_lock(&old_group->mutex);
		if (old_group->num_encoders_started) {
			pthread_mutex_unlock(&old_group->mutex);
			blog(LOG_ERROR, "obs_encoder_set_group: encoder '%s' existing group has started encoders",
			     obs_encoder_get_name(encoder));
			return false;
		}
		da_erase_item(old_group->encoders, &encoder);
		encoder->encoder_group = NULL;
		obs_encoder_release(encoder);
		pthread_mutex_unlock(&old_group->mutex);
	}

	if (!group)
		return true;

	pthread_mutex_lock(&group->mutex);

	if (group->num_encoders_started) {
		pthread_mutex_unlock(&group->mutex);
		blog(LOG_ERROR, "obs_encoder_set_group: specified group has started encoders");
		return false;
	}

	obs_encoder_t *ref = obs_encoder_get_ref(encoder);
	if (!ref) {
		pthread_mutex_unlock(&group->mutex);
		return false;
	}
	da_push_back(group->encoders, &ref);
	encoder->encoder_group = group;

	pthread_mutex_unlock(&group->mutex);

	return true;
}
