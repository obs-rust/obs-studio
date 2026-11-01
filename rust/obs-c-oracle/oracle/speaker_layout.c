/* Test-only oracle: the original
 * libobs/audio-monitoring/monitor-speaker-layout.c.h, unmodified, with its
 * function renamed to oracle_* so it can link next to the Rust shim, plus its
 * masks and the enum's size as the C sees them. It calls nothing outside the
 * file but get_audio_channels() (audio-io.h, static inline). */
#include <stddef.h>
#include <stdint.h>

#define audio_monitor_speaker_layout oracle_audio_monitor_speaker_layout

#include "audio-monitoring/monitor-speaker-layout.c.h"

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/speaker_layout.rs. */
enum {
	ORACLE_MONITOR_SPEAKER_LAYOUT_VALUE_COUNT = 6,
};

int64_t oracle_monitor_speaker_layout_value(size_t which);

int64_t oracle_monitor_speaker_layout_value(size_t which)
{
	const int64_t values[ORACLE_MONITOR_SPEAKER_LAYOUT_VALUE_COUNT] = {
		AUDIO_MONITOR_SPEAKER_MASK_2POINT1, AUDIO_MONITOR_SPEAKER_MASK_SURROUND,
		AUDIO_MONITOR_SPEAKER_MASK_4POINT1, AUDIO_MONITOR_SPEAKER_MASK_5POINT1,
		AUDIO_MONITOR_SPEAKER_MASK_7POINT1, sizeof(enum speaker_layout),
	};
	return which < ORACLE_MONITOR_SPEAKER_LAYOUT_VALUE_COUNT ? values[which] : -1;
}
