/* Test-only stand-in for libpulse's <pulse/def.h>, for the
 * pulseaudio-stream-flags oracle (oracle/pulse_stream_flags.c), so cargo
 * test needs no libpulse headers.
 *
 * Where the real header is installed (GCC and Clang see it with
 * __has_include_next), it is used instead and nothing below is defined:
 * the oracle then reads libpulse's own values, and the Rust constants are
 * checked against those. Otherwise (MSVC, macOS, a runner without
 * libpulse-dev) the subset the oracle needs is defined here, with the values
 * libpulse has had since 0.9. */

#pragma once

#if defined(__GNUC__)
/* #include_next is a GNU extension; -Wpedantic would flag it here. */
#pragma GCC system_header
#endif

#if defined(__has_include_next)
#if __has_include_next(<pulse/def.h>)
#include_next <pulse/def.h>
#define OBS_ORACLE_PULSE_DEF_REAL 1
#endif
#endif

#ifndef OBS_ORACLE_PULSE_DEF_REAL
#include <stdint.h>

#define PA_INVALID_INDEX ((uint32_t)-1)

typedef enum pa_stream_flags {
	PA_STREAM_NOFLAGS = 0x0000U,
	PA_STREAM_START_CORKED = 0x0001U,
	PA_STREAM_INTERPOLATE_TIMING = 0x0002U,
	PA_STREAM_AUTO_TIMING_UPDATE = 0x0008U,
	PA_STREAM_DONT_MOVE = 0x0200U,
} pa_stream_flags_t;

typedef enum pa_subscription_event_type {
	PA_SUBSCRIPTION_EVENT_SINK = 0x0000U,
	PA_SUBSCRIPTION_EVENT_SOURCE = 0x0001U,
	PA_SUBSCRIPTION_EVENT_SINK_INPUT = 0x0002U,
	PA_SUBSCRIPTION_EVENT_FACILITY_MASK = 0x000FU,
	PA_SUBSCRIPTION_EVENT_NEW = 0x0000U,
	PA_SUBSCRIPTION_EVENT_CHANGE = 0x0010U,
	PA_SUBSCRIPTION_EVENT_REMOVE = 0x0020U,
	PA_SUBSCRIPTION_EVENT_TYPE_MASK = 0x0030U,
} pa_subscription_event_type_t;
#endif
