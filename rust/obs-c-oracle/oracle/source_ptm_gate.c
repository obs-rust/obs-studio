/* Test-only oracle: the original libobs/obs-source-ptm-gate.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims. */
#define obs_source_ptm_gate_stop_time oracle_obs_source_ptm_gate_stop_time
#define obs_source_ptm_gate_held oracle_obs_source_ptm_gate_held
#define obs_source_ptm_gate_mutes oracle_obs_source_ptm_gate_mutes
#define obs_source_ptm_gate_talk_mutes oracle_obs_source_ptm_gate_talk_mutes

#include "obs-source-ptm-gate.c.h"

#undef obs_source_ptm_gate_stop_time
#undef obs_source_ptm_gate_held
#undef obs_source_ptm_gate_mutes
#undef obs_source_ptm_gate_talk_mutes

/* The header's constant, for the Rust side. */
unsigned long long oracle_source_ptm_gate_ns_per_ms(void);

unsigned long long oracle_source_ptm_gate_ns_per_ms(void)
{
	return OBS_SOURCE_PTM_GATE_NS_PER_MS;
}
