/* Original query and production adapter over an independent global pointer. */
#include <stddef.h>
#include <stdint.h>
#define obs core_initialized_oracle_core
#define obs_initialized oracle_obs_initialized
#include "obs-core-initialized.c.h"
#undef obs_initialized
#include "core_initialized_fields.inc"
#undef obs

struct obs_core *core_initialized_oracle_core;

void oracle_core_initialized_set(void *core);
void oracle_core_initialized_set(void *core)
{
	core_initialized_oracle_core = core;
}

void oracle_core_initialized_layout(size_t *layout);
void oracle_core_initialized_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_core *);
	layout[1] = _Alignof(struct obs_core *);
	layout[2] = sizeof(bool);
	layout[3] = _Alignof(bool);
}
