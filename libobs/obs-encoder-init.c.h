#include "libobs/build/libobs.pch.h"

static bool init_encoder(struct obs_encoder *encoder, const char *name, obs_data_t *settings, obs_data_t *hotkey_data)
{
	pthread_mutex_init_value(&encoder->init_mutex);
	pthread_mutex_init_value(&encoder->callbacks_mutex);
	pthread_mutex_init_value(&encoder->outputs_mutex);
	pthread_mutex_init_value(&encoder->pause.mutex);
	pthread_mutex_init_value(&encoder->roi_mutex);
	pthread_mutex_init_value(&encoder->last_error_mutex);

	if (!obs_context_data_init(&encoder->context, OBS_OBJ_TYPE_ENCODER, settings, name, NULL, hotkey_data, false))
		return false;
	if (pthread_mutex_init_recursive(&encoder->init_mutex) != 0)
		return false;
	if (pthread_mutex_init_recursive(&encoder->callbacks_mutex) != 0)
		return false;
	if (pthread_mutex_init(&encoder->outputs_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&encoder->pause.mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&encoder->roi_mutex, NULL) != 0)
		return false;
	if (pthread_mutex_init(&encoder->last_error_mutex, NULL) != 0)
		return false;

	if (encoder->orig_info.get_defaults) {
		encoder->orig_info.get_defaults(encoder->context.settings);
	}
	if (encoder->orig_info.get_defaults2) {
		encoder->orig_info.get_defaults2(encoder->context.settings, encoder->orig_info.type_data);
	}

	return true;
}
