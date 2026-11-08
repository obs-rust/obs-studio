/* Verbatim C queries; callbacks reach Rust without production C calls. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

obs_module_t *oracle_obs_get_module(const char *name);
obs_module_t *oracle_obs_get_disabled_module(const char *name);

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *module_name_lookup_core;
#else
static _Thread_local struct obs_core *module_name_lookup_core;
#endif
#define obs module_name_lookup_core
#define obs_get_module oracle_obs_get_module
#define obs_get_disabled_module oracle_obs_get_disabled_module
#include "libobs/obs-module-name-lookups.c.h"
#undef obs_get_module
#undef obs_get_disabled_module
#include "module_name_lookup_fields.inc"
#undef obs

typedef void *(*module_lookup_query)(const char *);
bool oracle_module_name_lookup_check(const char *const *names, size_t count, const char *needle, bool disabled,
				     module_lookup_query loaded, module_lookup_query inactive);
bool oracle_module_name_lookup_check(const char *const *names, size_t count, const char *needle, bool disabled,
				     module_lookup_query loaded, module_lookup_query inactive)
{
	struct obs_core core = {0};
	struct obs_module nodes[16] = {0};
	struct obs_module before[16];
	if (count > 16)
		abort();
	for (size_t i = 0; i < count; ++i) {
		size_t length = strlen(names[i]) + 1;
		nodes[i].mod_name = malloc(length);
		if (!nodes[i].mod_name)
			abort();
		memcpy(nodes[i].mod_name, names[i], length);
		nodes[i].next = i + 1 < count ? &nodes[i + 1] : NULL;
	}
	core.first_module = !disabled && count ? nodes : NULL;
	core.first_disabled_module = disabled && count ? nodes : NULL;
	memcpy(before, nodes, sizeof(nodes));
	module_name_lookup_core = &core;
	bool valid = obs_module_name_lookup_head_field(false) == &core.first_module &&
		     obs_module_name_lookup_head_field(true) == &core.first_disabled_module;
	for (size_t i = 0; i < count; ++i)
		valid = valid && obs_module_name_lookup_name_field(&nodes[i]) == &nodes[i].mod_name &&
			obs_module_name_lookup_next_field(&nodes[i]) == &nodes[i].next;
	valid = valid && loaded(needle) == oracle_obs_get_module(needle) &&
		inactive(needle) == oracle_obs_get_disabled_module(needle) &&
		memcmp(nodes, before, sizeof(nodes)) == 0 && core.first_module == (!disabled && count ? nodes : NULL) &&
		core.first_disabled_module == (disabled && count ? nodes : NULL);
	for (size_t i = 0; i < count; ++i) {
		valid = valid && strcmp(nodes[i].mod_name, names[i]) == 0;
		free(nodes[i].mod_name);
	}
	module_name_lookup_core = NULL;
	return valid;
}

void oracle_module_name_lookup_layout(size_t *layout);
void oracle_module_name_lookup_layout(size_t *layout)
{
	layout[0] = sizeof(((struct obs_module *)0)->mod_name);
	layout[1] = _Alignof(char *);
	layout[2] = sizeof(((struct obs_module *)0)->next);
	layout[3] = _Alignof(obs_module_t *);
}
