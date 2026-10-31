/* Test-only oracle: the original libobs/audio-monitoring/monitor-delay.c.h,
 * unmodified, with its five functions renamed to oracle_* so they can link
 * next to the Rust shims, plus its constants as the C sees them. It calls
 * nothing outside the file but llabs(). Nothing in it divides. */
#include <stddef.h>
#include <stdint.h>

#define audio_monitor_delay_stale oracle_audio_monitor_delay_stale
#define audio_monitor_delay_diff oracle_audio_monitor_delay_diff
#define audio_monitor_delay_unsynced oracle_audio_monitor_delay_unsynced
#define audio_monitor_delay_stalled oracle_audio_monitor_delay_stalled
#define audio_monitor_delay_judge_diff oracle_audio_monitor_delay_judge_diff

#include "audio-monitoring/monitor-delay.c.h"

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
