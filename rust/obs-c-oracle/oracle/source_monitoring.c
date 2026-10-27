/* Test-only oracle: the original libobs/obs-source-monitoring.c.h,
 * unmodified, with its thirteen functions renamed to oracle_* so they can
 * link next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_monitoring_changes oracle_obs_source_monitoring_changes
#define obs_source_monitoring_has_audio oracle_obs_source_monitoring_has_audio
#define obs_source_monitoring_mixers_store oracle_obs_source_monitoring_mixers_store
#define obs_source_monitoring_push_to_logs oracle_obs_source_monitoring_push_to_logs
#define obs_source_monitoring_on_type oracle_obs_source_monitoring_on_type
#define obs_source_monitoring_stored_type oracle_obs_source_monitoring_stored_type
#define obs_source_monitoring_monitor oracle_obs_source_monitoring_monitor
#define obs_source_monitoring_route oracle_obs_source_monitoring_route
#define obs_source_monitoring_off_reads_mute oracle_obs_source_monitoring_off_reads_mute
#define obs_source_monitoring_off_unmutes oracle_obs_source_monitoring_off_unmutes
#define obs_source_monitoring_only_mutes oracle_obs_source_monitoring_only_mutes
#define obs_source_monitoring_balance oracle_obs_source_monitoring_balance
#define obs_source_monitoring_audio_active oracle_obs_source_monitoring_audio_active

#include "obs-source-monitoring.c.h"

#undef obs_source_monitoring_audio_active
#undef obs_source_monitoring_balance
#undef obs_source_monitoring_only_mutes
#undef obs_source_monitoring_off_unmutes
#undef obs_source_monitoring_off_reads_mute
#undef obs_source_monitoring_route
#undef obs_source_monitoring_monitor
#undef obs_source_monitoring_stored_type
#undef obs_source_monitoring_on_type
#undef obs_source_monitoring_push_to_logs
#undef obs_source_monitoring_mixers_store
#undef obs_source_monitoring_has_audio
#undef obs_source_monitoring_changes

/* The values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_monitoring.rs; INT64_MIN past the end. The
 * renames above also renamed the monitor and route enum tags (they share
 * their functions' names), so the enums are sized by what the functions
 * return. */
int64_t oracle_source_monitoring_value(size_t which);

int64_t oracle_source_monitoring_value(size_t which)
{
	const int64_t values[] = {
		OBS_SOURCE_MONITORING_AUDIO,
		OBS_SOURCE_MONITORING_TYPE_NONE,
		OBS_SOURCE_MONITORING_TYPE_MONITOR_ONLY,
		OBS_SOURCE_MONITORING_TYPE_MONITOR_AND_OUTPUT,
		OBS_SOURCE_MONITORING_MONITOR_KEEP,
		OBS_SOURCE_MONITORING_MONITOR_CREATE,
		OBS_SOURCE_MONITORING_MONITOR_DESTROY,
		OBS_SOURCE_MONITORING_ROUTE_OFF,
		OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY,
		OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT,
		OBS_SOURCE_MONITORING_ROUTE_ON,
		OBS_SOURCE_MONITORING_ACTIVE_NONE,
		OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE,
		OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE,
		(int64_t)sizeof(oracle_obs_source_monitoring_monitor(false, false)),
		(int64_t)sizeof(oracle_obs_source_monitoring_route(0)),
		(int64_t)sizeof(oracle_obs_source_monitoring_audio_active(false, false)),
		(int64_t)sizeof(bool),
		(int64_t)sizeof(int),
		(int64_t)sizeof(float),
	};

	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
