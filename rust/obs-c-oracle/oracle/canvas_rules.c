/* Test-only oracle: the original libobs/obs-canvas-rules.c.h, unmodified,
 * with its seventeen functions renamed to oracle_* so they can link next to
 * the Rust shims. obs.h comes first for the values the port is checked
 * against. */
#include <stddef.h>
#include <stdint.h>

#include "obs.h"

#define obs_canvas_rules_main_flags oracle_obs_canvas_rules_main_flags
#define obs_canvas_rules_public_flags oracle_obs_canvas_rules_public_flags
#define obs_canvas_rules_main_view oracle_obs_canvas_rules_main_view
#define obs_canvas_rules_mix_audio oracle_obs_canvas_rules_mix_audio
#define obs_canvas_rules_public oracle_obs_canvas_rules_public
#define obs_canvas_rules_holds_scenes oracle_obs_canvas_rules_holds_scenes
#define obs_canvas_rules_saves oracle_obs_canvas_rules_saves
#define obs_canvas_rules_size_set oracle_obs_canvas_rules_size_set
#define obs_canvas_rules_format_set oracle_obs_canvas_rules_format_set
#define obs_canvas_rules_resets oracle_obs_canvas_rules_resets
#define obs_canvas_rules_global_signals oracle_obs_canvas_rules_global_signals
#define obs_canvas_rules_channel_ok oracle_obs_canvas_rules_channel_ok
#define obs_canvas_rules_name_empty oracle_obs_canvas_rules_name_empty
#define obs_canvas_rules_renames oracle_obs_canvas_rules_renames
#define obs_canvas_rules_removable oracle_obs_canvas_rules_removable
#define obs_canvas_rules_mark_removed oracle_obs_canvas_rules_mark_removed
#define obs_canvas_rules_removed oracle_obs_canvas_rules_removed

#include "obs-canvas-rules.c.h"

#undef obs_canvas_rules_removed
#undef obs_canvas_rules_mark_removed
#undef obs_canvas_rules_removable
#undef obs_canvas_rules_renames
#undef obs_canvas_rules_name_empty
#undef obs_canvas_rules_channel_ok
#undef obs_canvas_rules_global_signals
#undef obs_canvas_rules_resets
#undef obs_canvas_rules_format_set
#undef obs_canvas_rules_size_set
#undef obs_canvas_rules_saves
#undef obs_canvas_rules_holds_scenes
#undef obs_canvas_rules_public
#undef obs_canvas_rules_mix_audio
#undef obs_canvas_rules_main_view
#undef obs_canvas_rules_public_flags
#undef obs_canvas_rules_main_flags

/* The flags, values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/canvas_rules.rs; INT64_MIN past the end. */
int64_t oracle_canvas_rules_value(size_t which);

int64_t oracle_canvas_rules_value(size_t which)
{
	const int64_t values[] = {
		/* the header's copies */
		OBS_CANVAS_RULES_MAIN,
		OBS_CANVAS_RULES_ACTIVATE,
		OBS_CANVAS_RULES_MIX_AUDIO,
		OBS_CANVAS_RULES_SCENE_REF,
		OBS_CANVAS_RULES_EPHEMERAL,
		OBS_CANVAS_RULES_REMOVED,
		OBS_CANVAS_RULES_PUBLIC,
		OBS_CANVAS_RULES_FORMAT_NONE,
		OBS_CANVAS_RULES_MAX_CHANNELS,
		/* enum obs_canvas_flags (obs.h) */
		MAIN,
		ACTIVATE,
		MIX_AUDIO,
		SCENE_REF,
		EPHEMERAL,
		PROGRAM,
		PREVIEW,
		DEVICE,
		/* video-io.h and obs-defs.h */
		VIDEO_FORMAT_NONE,
		MAX_CHANNELS,
		/* sizes */
		sizeof(enum video_format),
		sizeof(int),
		sizeof(bool),
		sizeof(uint32_t),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
