/* Test-only oracle: the original libobs/obs-source-load.c.h, unmodified,
 * with its six functions renamed to oracle_* so they can link next to the
 * Rust shims, plus the constants and the layout of its struct as the C sees
 * them. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_load_prev_ver oracle_obs_source_load_prev_ver
#define obs_source_load_volume oracle_obs_source_load_volume
#define obs_source_load_balance oracle_obs_source_load_balance
#define obs_source_load_delay oracle_obs_source_load_delay
#define obs_source_load_deinterlace_mode oracle_obs_source_load_deinterlace_mode
#define obs_source_load_monitoring_migration oracle_obs_source_load_monitoring_migration

#include "obs-source-load.c.h"

#undef obs_source_load_monitoring_migration
#undef obs_source_load_deinterlace_mode
#undef obs_source_load_delay
#undef obs_source_load_balance
#undef obs_source_load_volume
#undef obs_source_load_prev_ver

/* In the order of rust/obs-c-oracle/src/source_load.rs. */
enum {
	ORACLE_SOURCE_LOAD_CONSTANT_COUNT = 7,
	ORACLE_SOURCE_LOAD_LAYOUT_COUNT = 6,
};

long long oracle_source_load_constant(size_t which);
size_t oracle_source_load_layout(size_t which);

long long oracle_source_load_constant(size_t which)
{
	const long long values[ORACLE_SOURCE_LOAD_CONSTANT_COUNT] = {
		OBS_SOURCE_LOAD_MONITOR_ONLY_BEFORE,     OBS_SOURCE_LOAD_MONITORING_ENABLED_BEFORE,
		OBS_SOURCE_LOAD_MONITOR_BY_DEFAULT,      OBS_SOURCE_LOAD_MONITORING_NONE,
		OBS_SOURCE_LOAD_MONITORING_MONITOR_ONLY, OBS_SOURCE_LOAD_ALL_MIXERS,
		OBS_SOURCE_LOAD_DEINTERLACE_DISABLE,
	};
	return which < ORACLE_SOURCE_LOAD_CONSTANT_COUNT ? values[which] : -1;
}

size_t oracle_source_load_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_LOAD_LAYOUT_COUNT] = {
		sizeof(struct obs_source_load_monitoring),
		_Alignof(struct obs_source_load_monitoring),
		offsetof(struct obs_source_load_monitoring, set_mixers),
		offsetof(struct obs_source_load_monitoring, mixers),
		offsetof(struct obs_source_load_monitoring, store_enabled),
		offsetof(struct obs_source_load_monitoring, enabled),
	};
	return which < ORACLE_SOURCE_LOAD_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
