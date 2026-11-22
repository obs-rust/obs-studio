#include "libobs/build/libobs.pch.h"

void obs_set_video_levels(float sdr_white_level, float hdr_nominal_peak_level)
{
	if (!obs_core_setup_rules_levels_core(obs != NULL)) {
		blog(LOG_WARNING, "obs_set_video_levels: no core, ignoring the levels");
		return;
	}

	struct obs_core_video *video = &obs->video;
	assert(video->graphics);

	/* obs_core_setup_rules_level_ok(): the draw paths divide by these, so
	 * a level that is not finite and greater than 0 is ignored and the
	 * previous one stays (#2209). */
	if (obs_core_setup_rules_level_ok(sdr_white_level))
		video->sdr_white_level = sdr_white_level;
	else
		blog(LOG_WARNING, "obs_set_video_levels: ignoring SDR white level %g, keeping %g",
		     (double)sdr_white_level, (double)video->sdr_white_level);

	if (obs_core_setup_rules_level_ok(hdr_nominal_peak_level))
		video->hdr_nominal_peak_level = hdr_nominal_peak_level;
	else
		blog(LOG_WARNING, "obs_set_video_levels: ignoring HDR nominal peak level %g, keeping %g",
		     (double)hdr_nominal_peak_level, (double)video->hdr_nominal_peak_level);
}
