#include "libobs/build/libobs.pch.h"

void obs_add_raw_video_callback(const struct video_scale_info *conversion,
				void (*callback)(void *param, struct video_data *frame), void *param)
{
	obs_add_raw_video_callback2(conversion, 1, callback, param);
}

void obs_add_raw_video_callback2(const struct video_scale_info *conversion, uint32_t frame_rate_divisor,
				 void (*callback)(void *param, struct video_data *frame), void *param)
{
	/* The main canvas has no mix until the first obs_reset_video() (#2253):
	 * there is no video to connect to, and the callback is not kept for
	 * later. */
	const obs_canvas_t *canvas = obs->data.main_canvas;
	struct obs_core_video_mix *video = canvas ? canvas->mix : NULL;
	if (!video) {
		blog(LOG_WARNING, "obs_add_raw_video_callback2: video is not set up yet, "
				  "the raw video callback was not added");
		return;
	}
	start_raw_video(video->video, conversion, frame_rate_divisor, callback, param);
}

void obs_remove_raw_video_callback(void (*callback)(void *param, struct video_data *frame), void *param)
{
	/* Before the first obs_reset_video() no callback can have been added. */
	const obs_canvas_t *canvas = obs->data.main_canvas;
	struct obs_core_video_mix *video = canvas ? canvas->mix : NULL;
	if (!video)
		return;
	stop_raw_video(video->video, callback, param);
}
