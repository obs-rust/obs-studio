#pragma once

#include <obs.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Applies a streaming service's encoder recommendations (keyframe interval,
 * rate control, maximum bitrate, B-frames, profile, x264 options) to the
 * video encoder settings of Advanced output mode.
 *
 * ignore_recommended is "Ignore streaming service setting recommendations"
 * (Stream1/IgnoreRecommended). With it, the user's encoder settings are kept
 * and only the service's keyframe interval is applied. It applies to the
 * streaming services list (rtmp_common) only: the settings a custom server
 * or WHIP need for their protocol are always applied. Either way, a keyframe
 * interval the user set shorter than the service's is kept.
 *
 * service_encoder_settings_apply_video() takes the recommendations from
 * service (obs_service_apply_encoder_settings()).
 * service_encoder_settings_apply_video_with() takes them from apply, which
 * gets param and the settings to change, for a service of type service_id. */
typedef void (*service_encoder_settings_apply_cb)(void *param, obs_data_t *video_settings);

void service_encoder_settings_apply_video_with(obs_data_t *settings, const char *service_id, bool ignore_recommended,
					       service_encoder_settings_apply_cb apply, void *param);
void service_encoder_settings_apply_video(obs_service_t *service, obs_data_t *settings, bool ignore_recommended);

#ifdef __cplusplus
}
#endif
