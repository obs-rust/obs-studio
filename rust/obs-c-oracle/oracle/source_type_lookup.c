/* Shared original lookup, instrumented without changing its ID semantics. */
#include "libobs/build/libobs.pch.h"

const struct obs_core *oracle_source_icon_core(void);
bool oracle_source_theme_icon_lookup(const char *id, struct obs_source_info **info);
struct obs_source_info *source_registry_original_find(const char *id);
#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *source_registry_core;
static __declspec(thread) size_t source_registry_lookups;
static __declspec(thread) const char *source_registry_last_id;
#else
static _Thread_local struct obs_core *source_registry_core;
static _Thread_local size_t source_registry_lookups;
static _Thread_local const char *source_registry_last_id;
#endif
static const struct obs_core *source_registry_current_core(void)
{
	return source_registry_core ? source_registry_core : oracle_source_icon_core();
}
#define obs_core_texture_activity_core source_registry_current_core
#define get_source_info source_registry_original_find
#include "libobs/obs-source-info-lookup.c.h"
#undef get_source_info
#undef obs_core_texture_activity_core

struct obs_core *oracle_source_registry_exchange_core(struct obs_core *core);
struct obs_core *oracle_source_registry_exchange_core(struct obs_core *core)
{
	struct obs_core *previous = source_registry_core;
	source_registry_core = core;
	return previous;
}
size_t oracle_source_registry_lookups(const char **last);
size_t oracle_source_registry_lookups(const char **last)
{
	*last = source_registry_last_id;
	return source_registry_lookups;
}
struct obs_source_info *get_source_info(const char *id)
{
	struct obs_source_info *theme_info;
	if (oracle_source_theme_icon_lookup(id, &theme_info))
		return theme_info;
	++source_registry_lookups;
	source_registry_last_id = id;
	return source_registry_original_find(id);
}
