#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_get_video(): the current main-canvas
 * field, its mix and the mix's video, each one borrowed read as the original
 * did, with no reference or lock taken. The layouts stay in C. */
struct obs_get_video_fields {
	const obs_canvas_t *canvas;
	const struct obs_core_video_mix *(*mix)(const obs_canvas_t *canvas);
	video_t *(*video)(const struct obs_core_video_mix *mix);
};

static const struct obs_core_video_mix *obs_get_video_canvas_mix(const obs_canvas_t *canvas)
{
	return canvas->mix;
}

static video_t *obs_get_video_mix_video(const struct obs_core_video_mix *mix)
{
	return mix->video;
}

struct obs_get_video_fields obs_get_video_fields(void);
struct obs_get_video_fields obs_get_video_fields(void)
{
	return (struct obs_get_video_fields){obs->data.main_canvas, obs_get_video_canvas_mix, obs_get_video_mix_video};
}
