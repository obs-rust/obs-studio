#pragma once

#include <stdbool.h>

struct audio_monitor;
struct obs_source;

/* The null audio monitoring backend (null-audio-monitoring.c), a private
 * adapter, not a new public API. It is Rust in hybrid libobs
 * (obs-audio-monitoring::null_monitoring) and the original C in the C
 * reference (null-audio-monitoring-reference.c). Hidden, not EXPORT.
 *
 * The canonical backend functions, which only the selected null backend
 * defines, forward to these uniquely named ones, so every build can link them
 * without clashing with another backend. None reads, writes, keeps or frees
 * what it is passed, logs or allocates: monitoring is unavailable, no device
 * is listed and no callback made, no monitor is created, reset and destroy do
 * nothing, and no two device IDs match. */
bool obs_null_audio_monitoring_available(void);
void obs_null_audio_monitoring_enum_devices(bool (*cb)(void *data, const char *name, const char *id), void *data);
struct audio_monitor *obs_null_audio_monitor_create(struct obs_source *source);
void obs_null_audio_monitor_reset(struct audio_monitor *monitor);
void obs_null_audio_monitor_destroy(struct audio_monitor *monitor);
bool obs_null_audio_monitoring_devices_match(const char *id1, const char *id2);
