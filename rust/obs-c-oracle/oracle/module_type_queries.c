/* Original C wrappers and classification dependencies, renamed independently. */
#define obs_is_core_module oracle_obs_is_core_module
#define obs_is_legacy_module oracle_obs_is_legacy_module
#define obs_module_load_rules_is_core oracle_obs_module_load_rules_is_core
#define obs_module_load_rules_is_legacy oracle_obs_module_load_rules_is_legacy
#include "libobs/obs-module-type-queries.c.h"
#undef obs_is_core_module
#undef obs_is_legacy_module
#undef obs_module_load_rules_is_core
#undef obs_module_load_rules_is_legacy
#include "module-type-query-fields.inc"

typedef bool (*module_type_query_t)(obs_module_t *);
bool oracle_module_type_queries_check(uint32_t word, module_type_query_t core, module_type_query_t legacy);
bool oracle_module_type_queries_check(uint32_t word, module_type_query_t core, module_type_query_t legacy)
{
	struct obs_module module = {0};
	module.module_type = (enum obs_runtime_module_type)word;
	return core(NULL) == oracle_obs_is_core_module(NULL) &&
	       legacy(NULL) == oracle_obs_is_legacy_module(NULL) &&
	       core(&module) == oracle_obs_is_core_module(&module) &&
	       legacy(&module) == oracle_obs_is_legacy_module(&module) &&
	       obs_module_type_query_field(&module) == &module.module_type &&
	       obs_module_type_query_field(NULL) == NULL;
}
void oracle_module_type_queries_layout(size_t *size, size_t *align);
void oracle_module_type_queries_layout(size_t *size, size_t *align)
{
	*size = sizeof(((struct obs_module *)0)->module_type);
	*align = _Alignof(enum obs_runtime_module_type);
}
