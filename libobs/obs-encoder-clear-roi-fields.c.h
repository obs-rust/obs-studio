#include "libobs/build/libobs.pch.h"

static void encoder_clear_roi_lock(void *mutex)
{
	pthread_mutex_lock(mutex);
}

static void encoder_clear_roi_unlock(void *mutex)
{
	pthread_mutex_unlock(mutex);
}

struct obs_encoder_clear_roi_fields obs_encoder_clear_roi_fields(obs_encoder_t *encoder)
{
	return (struct obs_encoder_clear_roi_fields){&encoder->roi.num, &encoder->roi_increment, &encoder->roi_mutex,
						     encoder_clear_roi_lock, encoder_clear_roi_unlock};
}
