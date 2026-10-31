/* Test-only oracle: the original libobs/audio-monitoring/monitor-delay.c.h
 * and monitor-video-sync.c.h, unmodified, with their six functions renamed
 * to oracle_* so they can link next to the Rust shims, plus their constants
 * as the C sees them. They call nothing outside the files but llabs().
 * Nothing in them divides. */
#include <stddef.h>
#include <stdint.h>

#define audio_monitor_delay_stale oracle_audio_monitor_delay_stale
#define audio_monitor_delay_diff oracle_audio_monitor_delay_diff
#define audio_monitor_delay_unsynced oracle_audio_monitor_delay_unsynced
#define audio_monitor_delay_stalled oracle_audio_monitor_delay_stalled
#define audio_monitor_delay_judge_diff oracle_audio_monitor_delay_judge_diff
#define audio_monitor_track_video_ts oracle_audio_monitor_track_video_ts

#include "audio-monitoring/monitor-delay.c.h"
#include "audio-monitoring/monitor-video-sync.c.h"

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/monitor_delay.rs. */
enum {
	ORACLE_MONITOR_DELAY_VALUE_COUNT = 8,
};

int64_t oracle_monitor_delay_value(size_t which);

int64_t oracle_monitor_delay_value(size_t which)
{
	const int64_t values[ORACLE_MONITOR_DELAY_VALUE_COUNT] = {
		AUDIO_MONITOR_DELAY_STALE_NS, AUDIO_MONITOR_DELAY_MAX_DIFF_NS,
		AUDIO_MONITOR_DELAY_SYNC_NS,  (int64_t)AUDIO_MONITOR_VIDEO_STALL_NS,
		AUDIO_MONITOR_DELAY_PLAY,     AUDIO_MONITOR_DELAY_HOLD,
		AUDIO_MONITOR_DELAY_DRAG,     sizeof(enum audio_monitor_delay_verdict),
	};
	return which < ORACLE_MONITOR_DELAY_VALUE_COUNT ? values[which] : -1;
}
