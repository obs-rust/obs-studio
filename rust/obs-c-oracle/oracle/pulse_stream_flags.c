/* Test-only oracle: the original
 * libobs/audio-monitoring/pulse/pulseaudio-stream-flags.c.h, unmodified,
 * with its three functions renamed to oracle_* so they can link next to the
 * Rust shims. <pulse/def.h> is libpulse's where installed, else the
 * stand-in in oracle/pulse-shim/. */
#include <stddef.h>
#include <stdint.h>

#define pulseaudio_monitor_stream_flags oracle_pulseaudio_monitor_stream_flags
#define pulseaudio_monitor_reset_on_sink_event oracle_pulseaudio_monitor_reset_on_sink_event
#define pulseaudio_monitor_backlog_excess oracle_pulseaudio_monitor_backlog_excess

#include "audio-monitoring/pulse/pulseaudio-stream-flags.c.h"

/* The <pulse/def.h> and pulseaudio-stream-flags.h values the C uses, in the
 * order of rust/obs-c-oracle/src/pulse_stream_flags.rs. */
enum {
	ORACLE_PULSE_DEF_COUNT = 13,
	ORACLE_PULSE_LAYOUT_COUNT = 4,
};

uint32_t oracle_pulse_def_value(size_t which);
uint64_t oracle_pulse_backlog_window(void);
uint64_t oracle_pulse_backlog_slack(void);
size_t oracle_pulse_backlog_layout(size_t which);
int oracle_pulse_def_is_real(void);

uint32_t oracle_pulse_def_value(size_t which)
{
	const uint32_t values[ORACLE_PULSE_DEF_COUNT] = {
		PA_STREAM_START_CORKED,
		PA_STREAM_INTERPOLATE_TIMING,
		PA_STREAM_AUTO_TIMING_UPDATE,
		PA_STREAM_DONT_MOVE,
		PA_SUBSCRIPTION_EVENT_SINK,
		PA_SUBSCRIPTION_EVENT_SOURCE,
		PA_SUBSCRIPTION_EVENT_SINK_INPUT,
		PA_SUBSCRIPTION_EVENT_FACILITY_MASK,
		PA_SUBSCRIPTION_EVENT_NEW,
		PA_SUBSCRIPTION_EVENT_CHANGE,
		PA_SUBSCRIPTION_EVENT_REMOVE,
		PA_SUBSCRIPTION_EVENT_TYPE_MASK,
		PA_INVALID_INDEX,
	};
	return which < ORACLE_PULSE_DEF_COUNT ? values[which] : 0;
}

uint64_t oracle_pulse_backlog_window(void)
{
	return PULSEAUDIO_MONITOR_BACKLOG_WINDOW;
}

uint64_t oracle_pulse_backlog_slack(void)
{
	return PULSEAUDIO_MONITOR_BACKLOG_SLACK;
}

size_t oracle_pulse_backlog_layout(size_t which)
{
	const size_t values[ORACLE_PULSE_LAYOUT_COUNT] = {
		sizeof(struct pulseaudio_monitor_backlog),
		_Alignof(struct pulseaudio_monitor_backlog),
		offsetof(struct pulseaudio_monitor_backlog, window_start),
		offsetof(struct pulseaudio_monitor_backlog, window_min),
	};
	return which < ORACLE_PULSE_LAYOUT_COUNT ? values[which] : 0;
}

/* 1 when <pulse/def.h> was libpulse's own, 0 for the stand-in. */
int oracle_pulse_def_is_real(void)
{
#ifdef OBS_ORACLE_PULSE_DEF_REAL
	return 1;
#else
	return 0;
#endif
}
