/* Test-only stand-in for libpulse's <pulse/channelmap.h>, for the
 * pulseaudio-monitor-format oracle (oracle/pulse_format.c). As
 * <pulse/sample.h> next to it: libpulse's own header where installed, else
 * the positions the C uses and pa_channel_map, with the values and layout
 * of libpulse's ABI. */

#pragma once

#if defined(__GNUC__)
/* #include_next is a GNU extension; -Wpedantic would flag it here. */
#pragma GCC system_header
#endif

#if defined(__has_include_next)
#if __has_include_next(<pulse/channelmap.h>)
#include_next <pulse/channelmap.h>
#define OBS_ORACLE_PULSE_CHANNELMAP_REAL 1
#endif
#endif

#ifndef OBS_ORACLE_PULSE_CHANNELMAP_REAL
#include <stdint.h>

#include <pulse/sample.h>

typedef enum pa_channel_position {
	PA_CHANNEL_POSITION_INVALID = -1,
	PA_CHANNEL_POSITION_MONO = 0,
	PA_CHANNEL_POSITION_FRONT_LEFT,
	PA_CHANNEL_POSITION_FRONT_RIGHT,
	PA_CHANNEL_POSITION_FRONT_CENTER,
	PA_CHANNEL_POSITION_REAR_CENTER,
	PA_CHANNEL_POSITION_REAR_LEFT,
	PA_CHANNEL_POSITION_REAR_RIGHT,
	PA_CHANNEL_POSITION_LFE,
	PA_CHANNEL_POSITION_FRONT_LEFT_OF_CENTER,
	PA_CHANNEL_POSITION_FRONT_RIGHT_OF_CENTER,
	PA_CHANNEL_POSITION_SIDE_LEFT,
	PA_CHANNEL_POSITION_SIDE_RIGHT,
	/* AUX0..AUX31 and the TOP_* positions follow up to 50. */
	PA_CHANNEL_POSITION_MAX = 51
} pa_channel_position_t;

typedef struct pa_channel_map {
	uint8_t channels;
	pa_channel_position_t map[PA_CHANNELS_MAX];
} pa_channel_map;
#endif
