#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_set_video_levels(): the core, its
 * graphics subsystem, the two levels and the three warnings, with the
 * original's format strings. The core layout and the log text stay in C;
 * Rust decides what is read, written and logged. Each reads the current
 * core; Rust asks for a field only after obs_set_video_levels_has_core(). */
bool obs_set_video_levels_has_core(void);
bool obs_set_video_levels_has_core(void)
{
	return obs != NULL;
}

bool obs_set_video_levels_has_graphics(void);
bool obs_set_video_levels_has_graphics(void)
{
	return obs->video.graphics != NULL;
}

float obs_set_video_levels_sdr_white_level(void);
float obs_set_video_levels_sdr_white_level(void)
{
	return obs->video.sdr_white_level;
}

void obs_set_video_levels_store_sdr_white_level(float level);
void obs_set_video_levels_store_sdr_white_level(float level)
{
	obs->video.sdr_white_level = level;
}

float obs_set_video_levels_hdr_nominal_peak_level(void);
float obs_set_video_levels_hdr_nominal_peak_level(void)
{
	return obs->video.hdr_nominal_peak_level;
}

void obs_set_video_levels_store_hdr_nominal_peak_level(float level);
void obs_set_video_levels_store_hdr_nominal_peak_level(float level)
{
	obs->video.hdr_nominal_peak_level = level;
}

void obs_set_video_levels_warn_no_core(void);
void obs_set_video_levels_warn_no_core(void)
{
	blog(LOG_WARNING, "obs_set_video_levels: no core, ignoring the levels");
}

void obs_set_video_levels_warn_sdr_white_level(float rejected, float kept);
void obs_set_video_levels_warn_sdr_white_level(float rejected, float kept)
{
	blog(LOG_WARNING, "obs_set_video_levels: ignoring SDR white level %g, keeping %g", (double)rejected,
	     (double)kept);
}

void obs_set_video_levels_warn_hdr_nominal_peak_level(float rejected, float kept);
void obs_set_video_levels_warn_hdr_nominal_peak_level(float rejected, float kept)
{
	blog(LOG_WARNING, "obs_set_video_levels: ignoring HDR nominal peak level %g, keeping %g", (double)rejected,
	     (double)kept);
}
