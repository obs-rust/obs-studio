/* Test-only oracle: the original
 * libobs/audio-monitoring/pulse/pulseaudio-monitor-steps.c.h, unmodified,
 * with its twenty functions renamed to oracle_* so they can link next to the
 * Rust shims, plus its constants as the C sees them. It calls
 * pulseaudio_monitor_stream_flags(), which is renamed too, so the C of
 * oracle/pulse_stream_flags.c answers. <pulse/def.h> and <pulse/sample.h>
 * are libpulse's where installed, else the stand-ins in oracle/pulse-shim/.
 * Nothing in it divides. */
#include <stddef.h>
#include <stdint.h>

#define pulseaudio_monitor_stream_flags oracle_pulseaudio_monitor_stream_flags

#define pulseaudio_monitor_grow oracle_pulseaudio_monitor_grow
#define pulseaudio_monitor_grown_tlength oracle_pulseaudio_monitor_grown_tlength
#define pulseaudio_monitor_uncork oracle_pulseaudio_monitor_uncork
#define pulseaudio_monitor_write_more oracle_pulseaudio_monitor_write_more
#define pulseaudio_monitor_write_short oracle_pulseaudio_monitor_write_short
#define pulseaudio_monitor_playback_idle oracle_pulseaudio_monitor_playback_idle
#define pulseaudio_monitor_playback_bytes oracle_pulseaudio_monitor_playback_bytes
#define pulseaudio_monitor_needs_volume oracle_pulseaudio_monitor_needs_volume
#define pulseaudio_monitor_sink_result oracle_pulseaudio_monitor_sink_result
#define pulseaudio_monitor_has_device oracle_pulseaudio_monitor_has_device
#define pulseaudio_monitor_self_monitor_check oracle_pulseaudio_monitor_self_monitor_check
#define pulseaudio_monitor_is_default oracle_pulseaudio_monitor_is_default
#define pulseaudio_monitor_fallback oracle_pulseaudio_monitor_fallback
#define pulseaudio_monitor_sink_named oracle_pulseaudio_monitor_sink_named
#define pulseaudio_monitor_format_invalid oracle_pulseaudio_monitor_format_invalid
#define pulseaudio_monitor_tlength_usec oracle_pulseaudio_monitor_tlength_usec
#define pulseaudio_monitor_open_flags oracle_pulseaudio_monitor_open_flags
#define pulseaudio_monitor_event_queued oracle_pulseaudio_monitor_event_queued
#define pulseaudio_monitor_context_good oracle_pulseaudio_monitor_context_good
#define pulseaudio_monitor_last oracle_pulseaudio_monitor_last

#include "audio-monitoring/pulse/pulseaudio-monitor-steps.c.h"

/* The constants the C uses, in the order of
 * rust/obs-c-oracle/src/pulse_monitor_steps.rs. */
enum {
	ORACLE_PULSE_MONITOR_VALUE_COUNT = 14,
};

int64_t oracle_pulse_monitor_value(size_t which);
size_t oracle_pulse_monitor_playback_bytes(uint64_t bytes_per_frame, uint32_t frames);

/* The frame size converted to uint_fast32_t as pulseaudio-output.c's
 * monitor->bytes_per_frame is: the host's width. */
size_t oracle_pulse_monitor_playback_bytes(uint64_t bytes_per_frame, uint32_t frames)
{
	return oracle_pulseaudio_monitor_playback_bytes((uint_fast32_t)bytes_per_frame, frames);
}

int64_t oracle_pulse_monitor_value(size_t which)
{
	const int64_t values[ORACLE_PULSE_MONITOR_VALUE_COUNT] = {
		PULSEAUDIO_MONITOR_DO_NOT_SELF_MONITOR,
		PULSEAUDIO_MONITOR_SINK_INFO,
		PULSEAUDIO_MONITOR_SINK_END,
		PULSEAUDIO_MONITOR_SINK_ERROR,
		PA_CONTEXT_UNCONNECTED,
		PA_CONTEXT_CONNECTING,
		PA_CONTEXT_AUTHORIZING,
		PA_CONTEXT_SETTING_NAME,
		PA_CONTEXT_READY,
		PA_CONTEXT_FAILED,
		PA_CONTEXT_TERMINATED,
		PA_SAMPLE_INVALID,
		(int64_t)sizeof(uint_fast32_t),
		(int64_t)sizeof(long),
	};
	return which < ORACLE_PULSE_MONITOR_VALUE_COUNT ? values[which] : 0;
}
