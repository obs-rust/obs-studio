/* Real C bodies and private-header layout baseline for #3839. */
#include "obs-internal.h"
#define obs_service_module_first oracle_obs_service_module_first
#define obs_service_module_next oracle_obs_service_module_next
#define obs_service_module_service_count oracle_obs_service_module_service_count
#define obs_service_module_service_id oracle_obs_service_module_service_id
#define obs_service_module_load_state oracle_obs_service_module_load_state
#define obs oracle_module_lookup_core
static struct obs_core module_lookup_core;
static struct obs_core *oracle_module_lookup_core = &module_lookup_core;
obs_module_t *obs_service_module_first(bool disabled);
obs_module_t *obs_service_module_next(obs_module_t *module);
size_t obs_service_module_service_count(obs_module_t *module);
const char *obs_service_module_service_id(obs_module_t *module, size_t i);
enum obs_module_load_state obs_service_module_load_state(obs_module_t *module);
#include "libobs/obs-service-module-lookup.c.h"

void obs_service_module_roots(obs_module_t **enabled, obs_module_t **disabled);
void obs_service_module_roots(obs_module_t **enabled, obs_module_t **disabled)
{
	*enabled = obs->first_module;
	*disabled = obs->first_disabled_module;
}
void oracle_module_lookup_set_roots(obs_module_t *enabled, obs_module_t *disabled);
void oracle_module_lookup_set_roots(obs_module_t *enabled, obs_module_t *disabled)
{
	obs->first_module = enabled;
	obs->first_disabled_module = disabled;
}
/* Name the real darray type: MSVC rejects anonymous definitions in offsetof. */
typedef DARRAY(char *) module_service_ids_t;

size_t oracle_module_lookup_layout(size_t index);
size_t oracle_module_lookup_layout(size_t index)
{
	const size_t values[] = {
		sizeof(struct obs_module),
		_Alignof(struct obs_module),
		offsetof(struct obs_module, mod_name),
		offsetof(struct obs_module, file),
		offsetof(struct obs_module, bin_path),
		offsetof(struct obs_module, data_path),
		offsetof(struct obs_module, module),
		offsetof(struct obs_module, loaded),
		offsetof(struct obs_module, load_state),
		offsetof(struct obs_module, load),
		offsetof(struct obs_module, unload),
		offsetof(struct obs_module, post_load),
		offsetof(struct obs_module, set_locale),
		offsetof(struct obs_module, get_string),
		offsetof(struct obs_module, free_locale),
		offsetof(struct obs_module, ver),
		offsetof(struct obs_module, set_pointer),
		offsetof(struct obs_module, name),
		offsetof(struct obs_module, description),
		offsetof(struct obs_module, author),
		offsetof(struct obs_module, metadata),
		offsetof(struct obs_module, next),
		offsetof(struct obs_module, sources),
		offsetof(struct obs_module, outputs),
		offsetof(struct obs_module, encoders),
		offsetof(struct obs_module, services),
		offsetof(struct obs_module, module_type),
		sizeof(((struct obs_module *)0)->services),
		_Alignof(module_service_ids_t),
		offsetof(module_service_ids_t, array),
		offsetof(module_service_ids_t, num),
		offsetof(module_service_ids_t, capacity),
	};
	return values[index];
}
