/* Test-only oracle: the original libobs/obs-source-transition-access.c.h,
 * unmodified, with its four functions renamed to oracle_* so they can link
 * next to the Rust shims (rust/obs-source-seam/src/ffi/source_transition_access.rs).
 *
 * Both sides reach the transition fields and lock through the same struct
 * obs_source: the C directly, the Rust shims through the transition seam,
 * libobs/obs-source-transition-seam.c.h, which
 * oracle/source_transition_settings.c compiles under its own names (with
 * transition_valid()), as the Rust shims call it in the hybrid libobs. The
 * start rules (obs_source_transition_start_active_index,
 * obs_source_transition_start_manual_reset) are the Rust ones, as in the
 * hybrid libobs; blog() is base-variadic.c.
 *
 * The C's calls into the rest of libobs go to recording stand-ins, which
 * the parity test hands the Rust shims too (their Calls):
 * obs_source_get_ref() to the slots oracle's oracle_slots_get_ref()
 * (oracle/source_transition_slots.c: a reference count per test source and
 * a trace), and obs_transition_set() to oracle_access_set() below, which
 * records the transition and the source it was handed. */

#define obs_transition_get_source oracle_obs_transition_get_source
#define obs_transition_get_active_source oracle_obs_transition_get_active_source
#define obs_transition_set_manual_torque oracle_obs_transition_set_manual_torque
#define obs_transition_set_manual_time oracle_obs_transition_set_manual_time
#define obs_source_get_ref oracle_slots_get_ref
#define obs_transition_set oracle_access_set

/* the access file's precompiled header brings obs.h, so the oracle_*
 * prototypes come from obs.h through the renames */
#include "obs-source-transition-access.c.h"

#undef obs_transition_set
#undef obs_source_get_ref
#undef obs_transition_set_manual_time
#undef obs_transition_set_manual_torque
#undef obs_transition_get_active_source
#undef obs_transition_get_source

#define ORACLE_ACCESS_SETS 16

static struct {
	obs_source_t *transition[ORACLE_ACCESS_SETS];
	obs_source_t *source[ORACLE_ACCESS_SETS];
	bool locked[ORACLE_ACCESS_SETS];
	size_t num;
} sets;

size_t oracle_access_sets(void);
void oracle_access_set_call(size_t i, obs_source_t **transition, obs_source_t **source, bool *locked);
void oracle_access_reset(void);

/* The stand-in for obs_transition_set(): records the call and whether the
 * transition's mutex was held (by a trylock). */
void oracle_access_set(obs_source_t *transition, obs_source_t *source)
{
	if (sets.num < ORACLE_ACCESS_SETS) {
		bool got = pthread_mutex_trylock(&transition->transition_mutex) == 0;
		if (got)
			pthread_mutex_unlock(&transition->transition_mutex);
		sets.transition[sets.num] = transition;
		sets.source[sets.num] = source;
		sets.locked[sets.num] = !got;
		sets.num++;
	}
}

size_t oracle_access_sets(void)
{
	return sets.num;
}

void oracle_access_set_call(size_t i, obs_source_t **transition, obs_source_t **source, bool *locked)
{
	*transition = sets.transition[i];
	*source = sets.source[i];
	*locked = sets.locked[i];
}

/* Forgets the recorded calls. */
void oracle_access_reset(void)
{
	memset(&sets, 0, sizeof(sets));
}
