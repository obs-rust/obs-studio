/* Test-only oracle: the original null audio monitoring backend
 * (libobs/audio-monitoring/null/null-audio-monitoring-reference.c.h, the C
 * reference's copy), unmodified, with its six functions renamed to oracle_*
 * so it can link next to the Rust shims. It reads nothing it is passed. */
#define obs_null_audio_monitoring_available oracle_null_audio_monitoring_available
#define obs_null_audio_monitoring_enum_devices oracle_null_audio_monitoring_enum_devices
#define obs_null_audio_monitor_create oracle_null_audio_monitor_create
#define obs_null_audio_monitor_reset oracle_null_audio_monitor_reset
#define obs_null_audio_monitor_destroy oracle_null_audio_monitor_destroy
#define obs_null_audio_monitoring_devices_match oracle_null_audio_monitoring_devices_match

#include "libobs/audio-monitoring/null/null-audio-monitoring-reference.c.h"
