/* Test-only oracle: the original libobs/obs-source-transition-settings.c.h,
 * unmodified, with its ten functions renamed to oracle_* so they can link
 * next to the Rust shims (rust/obs-source/src/ffi/source_transition_settings.rs).
 *
 * Both sides reach the transition fields through the same transition seam,
 * libobs/obs-source-transition-seam.c.h, compiled here under its own names:
 * it stays C in libobs.core-source, so the Rust shims call it in the test
 * binary as they do in the hybrid libobs. transition_valid() asks the Rust
 * type check and valid_scale_type() the Rust scale type rule
 * (obs_source_transition_load_*), and the save the Rust active index
 * (obs_source_transition_start_active_index) and the Rust obs_data, as in
 * the hybrid libobs; blog() is base-variadic.c.
 *
 * A test transition is a zeroed struct obs_source with its type, its
 * transition_mutex and up to two children that have only a name. */

#define obs_transition_is_active oracle_obs_transition_is_active
#define obs_transition_set_scale_type oracle_obs_transition_set_scale_type
#define obs_transition_get_scale_type oracle_obs_transition_get_scale_type
#define obs_transition_set_alignment oracle_obs_transition_set_alignment
#define obs_transition_get_alignment oracle_obs_transition_get_alignment
#define obs_transition_set_size oracle_obs_transition_set_size
#define obs_transition_get_size oracle_obs_transition_get_size
#define obs_transition_save oracle_obs_transition_save
#define obs_transition_enable_fixed oracle_obs_transition_enable_fixed
#define obs_transition_fixed oracle_obs_transition_fixed

/* the settings file's precompiled header brings obs.h, so the oracle_*
 * prototypes come from obs.h through the renames */
#include "obs-source-transition-settings.c.h"

#undef obs_transition_fixed
#undef obs_transition_enable_fixed
#undef obs_transition_save
#undef obs_transition_get_size
#undef obs_transition_set_size
#undef obs_transition_get_alignment
#undef obs_transition_set_alignment
#undef obs_transition_get_scale_type
#undef obs_transition_set_scale_type
#undef obs_transition_is_active

#include "obs-source-transition-seam.c.h"

/* A child kind: 0 no child, 1 a child named name, 2 a child whose
 * context.name is NULL. */
obs_source_t *oracle_transition_settings_new(int type, int child_a, const char *name_a, int child_b,
					     const char *name_b);
void oracle_transition_settings_free(obs_source_t *tr);

static obs_source_t *new_child(int kind, const char *name)
{
	obs_source_t *child;

	if (!kind)
		return NULL;
	child = bzalloc(sizeof(*child));
	child->context.name = kind == 1 ? bstrdup(name) : NULL;
	return child;
}

static void free_child(obs_source_t *child)
{
	if (!child)
		return;
	bfree(child->context.name);
	bfree(child);
}

obs_source_t *oracle_transition_settings_new(int type, int child_a, const char *name_a, int child_b,
					     const char *name_b)
{
	obs_source_t *tr = bzalloc(sizeof(*tr));

	tr->info.type = (enum obs_source_type)type;
	pthread_mutex_init(&tr->transition_mutex, NULL);
	tr->transition_sources[0] = new_child(child_a, name_a);
	tr->transition_sources[1] = new_child(child_b, name_b);
	return tr;
}

void oracle_transition_settings_free(obs_source_t *tr)
{
	free_child(tr->transition_sources[0]);
	free_child(tr->transition_sources[1]);
	pthread_mutex_destroy(&tr->transition_mutex);
	bfree(tr);
}

