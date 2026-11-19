#include "libobs/build/libobs.pch.h"

/* Real private field access for the Rust level queries (#5187). The Rust
 * shim checks the core for NULL before it asks for the video fields. */
struct obs_video_level_queries_fields {
	const graphics_t *graphics;
	float sdr_white_level;
	float hdr_nominal_peak_level;
};

const struct obs_core *obs_video_level_queries_core(void);
const struct obs_core *obs_video_level_queries_core(void)
{
	return obs;
}

struct obs_video_level_queries_fields obs_video_level_queries_fields(const struct obs_core *core);
struct obs_video_level_queries_fields obs_video_level_queries_fields(const struct obs_core *core)
{
	const struct obs_core_video *video = &core->video;
	return (struct obs_video_level_queries_fields){video->graphics, video->sdr_white_level,
						       video->hdr_nominal_peak_level};
}
