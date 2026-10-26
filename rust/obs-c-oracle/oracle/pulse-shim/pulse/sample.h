/* Test-only stand-in for libpulse's <pulse/sample.h>, for the
 * pulseaudio-monitor-format oracle (oracle/pulse_format.c), so cargo test
 * needs no libpulse headers.
 *
 * Where the real header is installed (GCC and Clang see it with
 * __has_include_next), it is used instead and nothing below is defined:
 * the oracle then reads libpulse's own values, and the Rust constants are
 * checked against those. Otherwise (MSVC, macOS, a runner without
 * libpulse-dev) the subset the oracle needs is defined here, with the values
 * of libpulse's ABI (S24_32 since 0.9.15). */

#pragma once

#if defined(__GNUC__)
/* #include_next is a GNU extension; -Wpedantic would flag it here. */
#pragma GCC system_header
#endif

#if defined(__has_include_next)
#if __has_include_next(<pulse/sample.h>)
#include_next <pulse/sample.h>
#define OBS_ORACLE_PULSE_SAMPLE_REAL 1
#endif
#endif

#ifndef OBS_ORACLE_PULSE_SAMPLE_REAL
#include <stdint.h>

#define PA_CHANNELS_MAX 32U

/* For the pulseaudio-monitor-steps oracle (oracle/pulse_monitor_steps.c). */
typedef uint64_t pa_usec_t;

typedef enum pa_sample_format {
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
	PA_SAMPLE_INVALID = -1
} pa_sample_format_t;
#endif
