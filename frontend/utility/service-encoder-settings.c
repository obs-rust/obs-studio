#include "service-encoder-settings.h"

void service_encoder_settings_apply_video_with(obs_data_t *settings, const char *service_id, bool ignore_recommended,
					       service_encoder_settings_apply_cb apply, void *param)
{
	(void)service_id;

	long long bitrate = obs_data_get_int(settings, "bitrate");
	long long keyint_sec = obs_data_get_int(settings, "keyint_sec");

	apply(param, settings);

	if (ignore_recommended)
		obs_data_set_int(settings, "bitrate", bitrate);

	long long enforced_keyint_sec = obs_data_get_int(settings, "keyint_sec");
	if (keyint_sec != 0 && keyint_sec < enforced_keyint_sec)
		obs_data_set_int(settings, "keyint_sec", keyint_sec);
}

static void apply_service(void *param, obs_data_t *video_settings)
{
	obs_service_apply_encoder_settings(param, video_settings, NULL);
}

void service_encoder_settings_apply_video(obs_service_t *service, obs_data_t *settings, bool ignore_recommended)
{
	service_encoder_settings_apply_video_with(settings, obs_service_get_id(service), ignore_recommended,
						  apply_service, service);
}
