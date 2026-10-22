/* Test-only oracle: the original
 * libobs/audio-monitoring/pulse/pulseaudio-monitor-format.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims. <pulse/sample.h> and <pulse/channelmap.h> are libpulse's where
 * installed, else the stand-ins in oracle/pulse-shim/. */
#include <stddef.h>
#include <stdint.h>

#define pulseaudio_channels_to_obs_speakers oracle_pulseaudio_channels_to_obs_speakers
#define pulseaudio_to_obs_audio_format oracle_pulseaudio_to_obs_audio_format
#define pulseaudio_channel_map oracle_pulseaudio_channel_map
#define pulseaudio_monitor_sink_format oracle_pulseaudio_monitor_sink_format
#define pulseaudio_monitor_sink_channels oracle_pulseaudio_monitor_sink_channels
#define pulseaudio_monitor_process_volume oracle_pulseaudio_monitor_process_volume
#define pulseaudio_monitor_queued_bytes oracle_pulseaudio_monitor_queued_bytes
#define pulseaudio_monitor_drop_split oracle_pulseaudio_monitor_drop_split
#define pulseaudio_monitor_devices_match oracle_pulseaudio_monitor_devices_match

#include "audio-monitoring/pulse/pulseaudio-monitor-format.c.h"

/* Fixed-width entry points for the two functions that take uint_fast*_t,
 * whose width differs by host (the Rust shims follow it, and the layout test
 * compares the widths below). The value is converted as a C caller's would
 * be. */
enum speaker_layout oracle_pulse_channels_to_speakers(uint64_t channels);
void oracle_pulse_process_volume(pa_sample_format_t format, uint64_t channels, float vol, uint8_t *const *resample_data,
				 uint32_t resample_frames);

enum speaker_layout oracle_pulse_channels_to_speakers(uint64_t channels)
{
	return oracle_pulseaudio_channels_to_obs_speakers((uint_fast32_t)channels);
}

void oracle_pulse_process_volume(pa_sample_format_t format, uint64_t channels, float vol, uint8_t *const *resample_data,
				 uint32_t resample_frames)
{
	oracle_pulseaudio_monitor_process_volume(format, (uint_fast8_t)channels, vol, resample_data, resample_frames);
}

/* The libpulse and libobs enum values the C uses, in the order of
 * rust/obs-c-oracle/src/pulse_format.rs. */
enum {
	ORACLE_PULSE_FORMAT_VALUE_COUNT = 41,
	ORACLE_PULSE_FORMAT_LAYOUT_COUNT = 10,
};

int64_t oracle_pulse_format_value(size_t which);
size_t oracle_pulse_format_layout(size_t which);
int oracle_pulse_format_headers_real(void);

int64_t oracle_pulse_format_value(size_t which)
{
	const int64_t values[ORACLE_PULSE_FORMAT_VALUE_COUNT] = {
		PA_SAMPLE_U8,
		PA_SAMPLE_ALAW,
		PA_SAMPLE_ULAW,
		PA_SAMPLE_S16LE,
		PA_SAMPLE_S16BE,
		PA_SAMPLE_FLOAT32LE,
		PA_SAMPLE_FLOAT32BE,
		PA_SAMPLE_S32LE,
		PA_SAMPLE_S32BE,
		PA_SAMPLE_S24LE,
		PA_SAMPLE_S24BE,
		PA_SAMPLE_S24_32LE,
		PA_SAMPLE_S24_32BE,
		PA_SAMPLE_MAX,
		PA_SAMPLE_INVALID,
		PA_CHANNELS_MAX,
		PA_CHANNEL_POSITION_INVALID,
		PA_CHANNEL_POSITION_MONO,
		PA_CHANNEL_POSITION_FRONT_LEFT,
		PA_CHANNEL_POSITION_FRONT_RIGHT,
		PA_CHANNEL_POSITION_FRONT_CENTER,
		PA_CHANNEL_POSITION_REAR_CENTER,
		PA_CHANNEL_POSITION_REAR_LEFT,
		PA_CHANNEL_POSITION_REAR_RIGHT,
		PA_CHANNEL_POSITION_LFE,
		PA_CHANNEL_POSITION_SIDE_LEFT,
		PA_CHANNEL_POSITION_SIDE_RIGHT,
		PA_CHANNEL_POSITION_MAX,
		AUDIO_FORMAT_UNKNOWN,
		AUDIO_FORMAT_U8BIT,
		AUDIO_FORMAT_16BIT,
		AUDIO_FORMAT_32BIT,
		AUDIO_FORMAT_FLOAT,
		SPEAKERS_UNKNOWN,
		SPEAKERS_MONO,
		SPEAKERS_STEREO,
		SPEAKERS_2POINT1,
		SPEAKERS_4POINT0,
		SPEAKERS_4POINT1,
		SPEAKERS_5POINT1,
		SPEAKERS_7POINT1,
	};
	return which < ORACLE_PULSE_FORMAT_VALUE_COUNT ? values[which] : INT64_MIN;
}

size_t oracle_pulse_format_layout(size_t which)
{
	const size_t values[ORACLE_PULSE_FORMAT_LAYOUT_COUNT] = {
		sizeof(pa_channel_map),
		_Alignof(pa_channel_map),
		offsetof(pa_channel_map, channels),
		offsetof(pa_channel_map, map),
		sizeof(struct pulseaudio_monitor_drop),
		_Alignof(struct pulseaudio_monitor_drop),
		offsetof(struct pulseaudio_monitor_drop, unwritten),
		offsetof(struct pulseaudio_monitor_drop, rewind),
		sizeof(uint_fast32_t),
		sizeof(uint_fast8_t),
	};
	return which < ORACLE_PULSE_FORMAT_LAYOUT_COUNT ? values[which] : 0;
}

/* 1 when <pulse/sample.h> and <pulse/channelmap.h> were libpulse's own, 0
 * for the stand-ins. */
int oracle_pulse_format_headers_real(void)
{
#if defined(OBS_ORACLE_PULSE_SAMPLE_REAL) && defined(OBS_ORACLE_PULSE_CHANNELMAP_REAL)
	return 1;
#else
	return 0;
#endif
}
