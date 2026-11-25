#include "libobs/build/libobs.pch.h"

#define encoder_active(encoder) os_atomic_load_bool(&encoder->active)

/* Private group-state copy seams; the encoder core owns their implementations. */
struct obs_encoder_group_reconfig group_reconfig_get(const struct obs_encoder_group *group);
void group_reconfig_set(struct obs_encoder_group *group, const struct obs_encoder_group_reconfig *reconfig);

void obs_encoder_update(obs_encoder_t *encoder, obs_data_t *settings)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_update"))
		return;

	obs_data_apply(encoder->context.settings, settings);

	// Encoder isn't initialized yet, only apply changes to settings
	if (!encoder->context.data)
		return;

	// Encoder doesn't support updates
	if (!encoder->info.update)
		return;

	if (!encoder_active(encoder)) {
		encoder->info.update(encoder->context.data, encoder->context.settings);
		return;
	}

	/* If the encoder is active we defer the update as it may not be reentrant. Setting reconfigure_requested to
	 * true makes the changes apply at the next possible moment in the encoder / GPU encoder thread.
	 */
	if (!encoder->encoder_group) {
		encoder->reconfigure_requested = true;
		return;
	}

	/* Encoder groups need slightly more special handling since encoders generally start new GOPs/insert IDR frames
	 * when reconfigured, so we need to reconfigure them all on the same frame.
	 */
	struct obs_encoder_group *group = encoder->encoder_group;
	pthread_mutex_lock(&group->mutex);
	struct obs_encoder_group_reconfig reconfig = group_reconfig_get(group);
	obs_encoder_group_reconfig_update(&reconfig, group->encoders.num, encoder->last_handled_reconfigure_request);
	group_reconfig_set(group, &reconfig);
	pthread_mutex_unlock(&group->mutex);
}

#undef encoder_active
