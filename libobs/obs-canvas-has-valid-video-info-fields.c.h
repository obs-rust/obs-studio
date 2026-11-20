#include "libobs/build/libobs.pch.h"

/* Private adapter for the Rust obs_canvas_has_valid_video_info(): the
 * canvas's stored video info, canvas->ovi. The struct layout stays in C; the
 * check reads its fields directly and unlocked, as the original did. A NULL
 * canvas is outside the C valid domain and is not checked. */
const struct obs_video_info *obs_canvas_has_valid_video_info_ovi(const obs_canvas_t *canvas);
const struct obs_video_info *obs_canvas_has_valid_video_info_ovi(const obs_canvas_t *canvas)
{
	return &canvas->ovi;
}
