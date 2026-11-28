#include "libobs/build/libobs.pch.h"

static void encoder_roi_list_lock(void *mutex)
{
	pthread_mutex_lock(mutex);
}

static void encoder_roi_list_unlock(void *mutex)
{
	pthread_mutex_unlock(mutex);
}

static void encoder_roi_list_video_size(void *media, uint32_t *width, uint32_t *height)
{
	*width = video_output_get_width(media);
	*height = video_output_get_height(media);
}

struct obs_encoder_roi_list_fields obs_encoder_roi_list_fields(obs_encoder_t *encoder)
{
	return (struct obs_encoder_roi_list_fields){&encoder->roi.da,        &encoder->roi_increment,
						    &encoder->info.caps,     &encoder->scaled_width,
						    &encoder->scaled_height, &encoder->media,
						    &encoder->roi_mutex,     encoder_roi_list_lock,
						    encoder_roi_list_unlock, encoder_roi_list_video_size};
}
