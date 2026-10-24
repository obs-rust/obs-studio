/* Test-only oracle: the original libobs/obs-source-media-admission.c.h,
 * unmodified, with its eight functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#include "obs.h"

#define obs_source_media_admit oracle_obs_source_media_admit
#define obs_source_media_admit_flags oracle_obs_source_media_admit_flags
#define obs_source_media_admit_callback oracle_obs_source_media_admit_callback
#define obs_source_media_time_fallback oracle_obs_source_media_time_fallback
#define obs_source_media_state_fallback oracle_obs_source_media_state_fallback
#define obs_source_media_dispatch oracle_obs_source_media_dispatch
#define obs_source_media_signal_reads_pause oracle_obs_source_media_signal_reads_pause
#define obs_source_media_signal oracle_obs_source_media_signal

#include "obs-source-media-admission.c.h"

#undef obs_source_media_signal
#undef obs_source_media_signal_reads_pause
#undef obs_source_media_dispatch
#undef obs_source_media_state_fallback
#undef obs_source_media_time_fallback
#undef obs_source_media_admit_callback
#undef obs_source_media_admit_flags
#undef obs_source_media_admit

/* The values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_media_admission.rs; INT64_MIN past the end.
 * The libobs values the header mirrors come from obs.h (enum
 * media_action_type is in obs-internal.h, which obs-source.c checks). The
 * renames above also renamed the enum tags that share a function's name,
 * enum obs_source_media_admit and enum obs_source_media_dispatch, so their
 * sizes are taken under the oracle_ tag. */
int64_t oracle_source_media_admission_value(size_t which);

int64_t oracle_source_media_admission_value(size_t which)
{
	const int64_t values[] = {
		/* the mirrors and the libobs values */
		OBS_SOURCE_MEDIA_CONTROLLABLE,
		OBS_SOURCE_CONTROLLABLE_MEDIA,
		OBS_SOURCE_MEDIA_STATE_NONE,
		OBS_MEDIA_STATE_NONE,
		OBS_SOURCE_MEDIA_ACTION_NONE,
		OBS_SOURCE_MEDIA_ACTION_PLAY_PAUSE,
		OBS_SOURCE_MEDIA_ACTION_RESTART,
		OBS_SOURCE_MEDIA_ACTION_STOP,
		OBS_SOURCE_MEDIA_ACTION_NEXT,
		OBS_SOURCE_MEDIA_ACTION_PREVIOUS,
		OBS_SOURCE_MEDIA_ACTION_SET_TIME,
		/* enum obs_source_media_call */
		OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE,
		OBS_SOURCE_MEDIA_CALL_RESTART,
		OBS_SOURCE_MEDIA_CALL_STOP,
		OBS_SOURCE_MEDIA_CALL_NEXT,
		OBS_SOURCE_MEDIA_CALL_PREVIOUS,
		OBS_SOURCE_MEDIA_CALL_GET_DURATION,
		OBS_SOURCE_MEDIA_CALL_GET_TIME,
		OBS_SOURCE_MEDIA_CALL_SET_TIME,
		OBS_SOURCE_MEDIA_CALL_GET_STATE,
		OBS_SOURCE_MEDIA_CALL_STARTED,
		OBS_SOURCE_MEDIA_CALL_ENDED,
		/* enum obs_source_media_admit */
		OBS_SOURCE_MEDIA_ADMIT_REJECT,
		OBS_SOURCE_MEDIA_ADMIT_ACCEPT,
		OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS,
		OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK,
		/* enum obs_source_media_dispatch */
		OBS_SOURCE_MEDIA_DISPATCH_RETURN,
		OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE,
		OBS_SOURCE_MEDIA_DISPATCH_RESTART,
		OBS_SOURCE_MEDIA_DISPATCH_STOP,
		OBS_SOURCE_MEDIA_DISPATCH_NEXT,
		OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS,
		OBS_SOURCE_MEDIA_DISPATCH_SET_TIME,
		OBS_SOURCE_MEDIA_DISPATCH_SKIP,
		/* sizes */
		sizeof(enum obs_source_media_call),
		sizeof(enum oracle_obs_source_media_admit),
		sizeof(enum oracle_obs_source_media_dispatch),
		sizeof(enum obs_media_state),
		sizeof(int),
		sizeof(bool),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
