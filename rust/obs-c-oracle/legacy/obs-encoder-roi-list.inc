bool obs_encoder_add_roi(obs_encoder_t *encoder, const struct obs_encoder_roi *roi)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_add_roi"))
		return false;
	if (!roi)
		return false;
	if (!obs_encoder_roi_admit(encoder->info.caps, roi))
		return false;

	pthread_mutex_lock(&encoder->roi_mutex);
	da_push_back(encoder->roi, roi);
	encoder->roi_increment++;
	pthread_mutex_unlock(&encoder->roi_mutex);

	return true;
}

void obs_encoder_enum_roi(obs_encoder_t *encoder, void (*enum_proc)(void *, struct obs_encoder_roi *), void *param)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_enum_roi"))
		return;

	float scale_x = 0;
	float scale_y = 0;

	/* Scale ROI passed to callback to output size */
	if (encoder->scaled_height && encoder->scaled_width) {
		const uint32_t width = video_output_get_width(encoder->media);
		const uint32_t height = video_output_get_height(encoder->media);

		if (!obs_encoder_roi_scale_factors(encoder->scaled_width, encoder->scaled_height, width, height,
						   &scale_x, &scale_y))
			return;
	}

	pthread_mutex_lock(&encoder->roi_mutex);

	size_t idx = encoder->roi.num;
	while (idx) {
		struct obs_encoder_roi *roi = &encoder->roi.array[--idx];

		if (scale_x > 0 && scale_y > 0) {
			struct obs_encoder_roi scaled_roi;
			obs_encoder_roi_scale(roi, scale_x, scale_y, &scaled_roi);

			enum_proc(param, &scaled_roi);
		} else {
			enum_proc(param, roi);
		}
	}

	pthread_mutex_unlock(&encoder->roi_mutex);
}
