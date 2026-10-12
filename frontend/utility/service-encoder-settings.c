#include "service-encoder-settings.h"

#include <string.h>

/* Only the streaming services list (rtmp-services' rtmp_common) has
 * recommendations. What a custom server or WHIP apply is what their
 * protocol needs, such as repeated headers or no B-frames. */
static bool has_recommendations(const char *service_id)
{
	return service_id && strcmp(service_id, "rtmp_common") == 0;
}

void service_encoder_settings_apply_video_with(obs_data_t *settings, const char *service_id, bool ignore_recommended,
					       service_encoder_settings_apply_cb apply, void *param)
{
	long long keyint_sec = obs_data_get_int(settings, "keyint_sec");
	long long enforced_keyint_sec;

	if (ignore_recommended && has_recommendations(service_id)) {
		/* Keep every encoder setting the user chose (rate control,
		 * bitrate, buffer size, B-frames, profile, x264 options) and
		 * only take the service's keyframe interval. */
		obs_data_t *recommended = obs_data_create();
		obs_data_set_int(recommended, "keyint_sec", keyint_sec);
		apply(param, recommended);
		enforced_keyint_sec = obs_data_get_int(recommended, "keyint_sec");
		obs_data_release(recommended);
	} else {
		apply(param, settings);
		enforced_keyint_sec = obs_data_get_int(settings, "keyint_sec");
	}

	/* A keyframe interval shorter than the service's is kept. */
	if (keyint_sec == 0 || keyint_sec > enforced_keyint_sec)
		keyint_sec = enforced_keyint_sec;
	if (obs_data_get_int(settings, "keyint_sec") != keyint_sec)
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
