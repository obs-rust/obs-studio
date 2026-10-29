/* Test-only oracle: the original libobs/obs-core-setup-rules.c.h,
 * unmodified, with its fifteen entry points renamed to oracle_* so they
 * can link next to the Rust shims. Also the constants and enum values of
 * obs-core-setup-rules.h, and the obs-defs.h, obs-source.h and audio-io.h
 * values they copy, as the C sees them. */

#include <stddef.h>
#include <stdint.h>

#include "obs.h"

#define obs_core_setup_rules_video_gate oracle_obs_core_setup_rules_video_gate
#define obs_core_setup_rules_audio_refused oracle_obs_core_setup_rules_audio_refused
#define obs_core_setup_rules_buffer_set oracle_obs_core_setup_rules_buffer_set
#define obs_core_setup_rules_buffer_ticks oracle_obs_core_setup_rules_buffer_ticks
#define obs_core_setup_rules_buffer_default_ticks oracle_obs_core_setup_rules_buffer_default_ticks
#define obs_core_setup_rules_cmdline_set oracle_obs_core_setup_rules_cmdline_set
#define obs_core_setup_rules_cmdline_len oracle_obs_core_setup_rules_cmdline_len
#define obs_core_setup_rules_cmdline_table oracle_obs_core_setup_rules_cmdline_table
#define obs_core_setup_rules_data_file_found oracle_obs_core_setup_rules_data_file_found
#define obs_core_setup_rules_data_path_match oracle_obs_core_setup_rules_data_path_match
#define obs_core_setup_rules_latest_newer oracle_obs_core_setup_rules_latest_newer
#define obs_core_setup_rules_aoc_check oracle_obs_core_setup_rules_aoc_check
#define obs_core_setup_rules_monitor_missing oracle_obs_core_setup_rules_monitor_missing
#define obs_core_setup_rules_monitor_same oracle_obs_core_setup_rules_monitor_same
#define obs_core_setup_rules_mix_step oracle_obs_core_setup_rules_mix_step

#include "obs-core-setup-rules.c.h"

#undef obs_core_setup_rules_mix_step
#undef obs_core_setup_rules_monitor_same
#undef obs_core_setup_rules_monitor_missing
#undef obs_core_setup_rules_aoc_check
#undef obs_core_setup_rules_latest_newer
#undef obs_core_setup_rules_data_path_match
#undef obs_core_setup_rules_data_file_found
#undef obs_core_setup_rules_cmdline_table
#undef obs_core_setup_rules_cmdline_len
#undef obs_core_setup_rules_cmdline_set
#undef obs_core_setup_rules_buffer_default_ticks
#undef obs_core_setup_rules_buffer_ticks
#undef obs_core_setup_rules_buffer_set
#undef obs_core_setup_rules_audio_refused
#undef obs_core_setup_rules_video_gate

int64_t oracle_core_setup_rules_constant(size_t i);

/* In the order of CONSTANT_NAMES (src/core_setup_rules.rs). */
static const int64_t constants[] = {
	OBS_CORE_SETUP_RULES_VIDEO_GO,
	OBS_CORE_SETUP_RULES_VIDEO_FAIL,
	OBS_CORE_SETUP_RULES_VIDEO_INVALID_PARAM,
	OBS_CORE_SETUP_RULES_VIDEO_CURRENTLY_ACTIVE,
	OBS_VIDEO_SUCCESS,
	OBS_VIDEO_FAIL,
	OBS_VIDEO_INVALID_PARAM,
	OBS_VIDEO_CURRENTLY_ACTIVE,
	OBS_CORE_SETUP_RULES_DO_NOT_SELF_MONITOR,
	OBS_SOURCE_DO_NOT_SELF_MONITOR,
	OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES,
	AUDIO_OUTPUT_FRAMES,
	OBS_CORE_SETUP_RULES_DEFAULT_TICKS,
	OBS_CORE_SETUP_RULES_LATEST_NONE,
	OBS_CORE_SETUP_RULES_VIDEO_CORE,
	OBS_CORE_SETUP_RULES_VIDEO_ACTIVE,
	OBS_CORE_SETUP_RULES_VIDEO_STARTING,
	OBS_CORE_SETUP_RULES_VIDEO_OUTPUT_SIZE,
	OBS_CORE_SETUP_RULES_VIDEO_BASE_SIZE,
	OBS_CORE_SETUP_RULES_AUDIO_CORE,
	OBS_CORE_SETUP_RULES_AUDIO_ACTIVE,
	OBS_CORE_SETUP_RULES_MIX_MAIN,
	OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY,
	OBS_CORE_SETUP_RULES_MIX_VIEW,
	OBS_CORE_SETUP_RULES_MIX_VIDEO,
	OBS_CORE_SETUP_RULES_MIX_COUNT,
	OBS_CORE_SETUP_RULES_MIX_SKIP,
	(int64_t)sizeof(enum obs_core_setup_rules_video_step),
	(int64_t)sizeof(enum obs_core_setup_rules_audio_step),
	(int64_t)sizeof(enum obs_core_setup_rules_mix),
	(int64_t)sizeof(void *),
};

int64_t oracle_core_setup_rules_constant(size_t i)
{
	return i < sizeof(constants) / sizeof(constants[0]) ? constants[i] : INT64_MIN;
}
