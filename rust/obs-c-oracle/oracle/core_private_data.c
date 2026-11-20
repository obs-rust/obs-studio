/* Original wrappers and the real field adapter over a test-owned real core.
 * Data operations are dependencies shared with Rust; this oracle isolates the
 * two wrappers, while unchanged Tier 2 also runs the complete C data backend. */
#include "libobs/build/libobs.pch.h"

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core private_data_core;
#else
static _Thread_local struct obs_core private_data_core;
#endif
static struct obs_core *private_data_current(void)
{
	return &private_data_core;
}

void oracle_core_private_data_set(obs_data_t *data);
void oracle_core_private_data_set(obs_data_t *data)
{
	private_data_core.data.private_data = data;
}

obs_data_t *oracle_obs_get_private_data(void);
void oracle_obs_set_private_data(obs_data_t *settings);
void oracle_obs_apply_private_data(obs_data_t *settings);
#define obs private_data_current()
#define obs_get_private_data oracle_obs_get_private_data
#define obs_set_private_data oracle_obs_set_private_data
#include "libobs/obs-core-private-data.c.h"
#undef obs_set_private_data
#undef obs_get_private_data
#define obs_apply_private_data oracle_obs_apply_private_data
#include "libobs/obs-apply-private-data.c.h"
#undef obs_apply_private_data
#include "core_private_data_fields.inc"
#undef obs
