/* Original C queries and real-header fixtures share the ported source lookup
 * over a per-thread registry (source_info_lookup.c). */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#define obs_source_get_display_name oracle_obs_source_get_display_name
#define obs_get_source_output_flags oracle_obs_get_source_output_flags
#define obs_is_source_configurable oracle_obs_is_source_configurable
#include "libobs/obs-source-type-info.c.h"
#undef obs_source_get_display_name
#undef obs_get_source_output_flags
#undef obs_is_source_configurable
#include "source_type_info_fields.inc"

struct obs_core *oracle_source_registry_exchange_core(struct obs_core *core);
size_t oracle_source_registry_lookups(void);
struct sti_trace {
	const char *result;
	void *last;
	size_t calls;
	bool unexpected;
};
#if defined(_MSC_VER)
static __declspec(thread) struct sti_trace *sti_active;
#else
static _Thread_local struct sti_trace *sti_active;
#endif
static const char *sti_name(void *data)
{
	sti_active->last = data;
	++sti_active->calls;
	return sti_active->result;
}
static obs_properties_t *sti_properties(void *data)
{
	(void)data;
	sti_active->unexpected = true;
	return NULL;
}
static obs_properties_t *sti_properties2(void *data, void *type_data)
{
	(void)type_data;
	return sti_properties(data);
}
static void sti_defaults(obs_data_t *settings)
{
	(void)settings;
	sti_active->unexpected = true;
}
static void sti_defaults2(void *type_data, obs_data_t *settings)
{
	(void)type_data;
	sti_defaults(settings);
}
typedef const char *(*sti_name_query)(const char *id);
typedef uint32_t (*sti_flags_query)(const char *id);
typedef bool (*sti_configurable_query)(const char *id);
/* One lookup per query, seen by the shared fixture as it starts; a NULL ID
 * reads no registry, so it is not seen (source_info_lookup.c). */
static bool sti_one_lookup(size_t before, const char *query)
{
	return oracle_source_registry_lookups() == before + (query != NULL);
}

bool oracle_source_type_info_check(const char *const *ids, const uint32_t *flags, const unsigned char *masks,
				   size_t count, const char *query, const char *const *results, size_t result_count,
				   unsigned null_data, bool names, sti_name_query name, sti_flags_query output_flags,
				   sti_configurable_query configurable);
bool oracle_source_type_info_check(const char *const *ids, const uint32_t *flags, const unsigned char *masks,
				   size_t count, const char *query, const char *const *results, size_t result_count,
				   unsigned null_data, bool names, sti_name_query name, sti_flags_query output_flags,
				   sti_configurable_query configurable)
{
	if (count > 8 || result_count > 8)
		return false;
	struct obs_core core = {0}, before_core;
	struct obs_source_info entries[8] = {0}, before[8];
	unsigned tokens[8] = {0};
	struct sti_trace trace = {0};
	struct sti_trace *previous_trace = sti_active;
	sti_active = &trace;
	for (size_t i = 0; i < count; ++i) {
		entries[i].id = ids[i];
		entries[i].type = OBS_SOURCE_TYPE_INPUT;
		entries[i].output_flags = flags[i];
		entries[i].get_name = names ? sti_name : NULL;
		entries[i].type_data = null_data & (1u << i) ? NULL : &tokens[i];
		entries[i].get_properties = masks[i] & 1 ? sti_properties : NULL;
		entries[i].get_properties2 = masks[i] & 2 ? sti_properties2 : NULL;
		entries[i].get_defaults = masks[i] & 4 ? sti_defaults : NULL;
		entries[i].get_defaults2 = masks[i] & 8 ? sti_defaults2 : NULL;
	}
	core.source_types.array = entries;
	core.source_types.num = count;
	core.source_types.capacity = 8;
	memcpy(before, entries, sizeof(entries));
	memcpy(&before_core, &core, sizeof(core));
	struct obs_core *previous = oracle_source_registry_exchange_core(&core);
	bool valid = true;
	for (size_t i = 0; i < count; ++i) {
		struct obs_source_type_info_fields fields = obs_source_type_info_fields(&entries[i]);
		valid = valid && fields.get_name == entries[i].get_name && fields.type_data == entries[i].type_data &&
			fields.output_flags == entries[i].output_flags &&
			fields.has_properties == (entries[i].get_properties != NULL) &&
			fields.has_properties2 == (entries[i].get_properties2 != NULL);
	}
	for (size_t i = 0; i < result_count; ++i) {
		trace = (struct sti_trace){.result = results[i]};
		size_t lookups = oracle_source_registry_lookups();
		uint32_t rust_flags = output_flags(query);
		valid = valid && sti_one_lookup(lookups, query);
		uint32_t c_flags = oracle_obs_get_source_output_flags(query);
		valid = valid && rust_flags == c_flags && trace.calls == 0;
		lookups = oracle_source_registry_lookups();
		bool rust_configurable = configurable(query);
		valid = valid && sti_one_lookup(lookups, query);
		bool c_configurable = oracle_obs_is_source_configurable(query);
		valid = valid && rust_configurable == c_configurable && trace.calls == 0;
		if (names) {
			lookups = oracle_source_registry_lookups();
			const char *rust_name = name(query);
			valid = valid && sti_one_lookup(lookups, query);
			size_t rust_calls = trace.calls;
			void *rust_data = trace.last;
			trace.calls = 0;
			trace.last = NULL;
			const char *c_name = oracle_obs_source_get_display_name(query);
			valid = valid && rust_name == c_name && rust_calls == trace.calls && rust_data == trace.last &&
				trace.calls <= 1;
		}
		valid = valid && !trace.unexpected && memcmp(entries, before, sizeof(entries)) == 0 &&
			memcmp(&core, &before_core, sizeof(core)) == 0;
	}
	oracle_source_registry_exchange_core(previous);
	sti_active = previous_trace;
	return valid;
}

void oracle_source_type_info_layout(size_t *out);
void oracle_source_type_info_layout(size_t *out)
{
	out[0] = sizeof(struct obs_source_type_info_fields);
	out[1] = _Alignof(struct obs_source_type_info_fields);
	out[2] = offsetof(struct obs_source_type_info_fields, get_name);
	out[3] = offsetof(struct obs_source_type_info_fields, type_data);
	out[4] = offsetof(struct obs_source_type_info_fields, output_flags);
	out[5] = offsetof(struct obs_source_type_info_fields, has_properties);
	out[6] = offsetof(struct obs_source_type_info_fields, has_properties2);
}

bool oracle_source_type_info_mutable(sti_name_query name);
bool oracle_source_type_info_mutable(sti_name_query name)
{
	char text[] = "n\xff\0x";
	struct sti_trace trace = {.result = text};
	struct sti_trace *previous_trace = sti_active;
	sti_active = &trace;
	struct obs_source_info info = {.id = "mutable", .get_name = sti_name, .type_data = &trace};
	struct obs_core core = {0};
	core.source_types.array = &info;
	core.source_types.num = 1;
	struct obs_core *previous = oracle_source_registry_exchange_core(&core);
	const char *rust = name(info.id);
	const char *original = oracle_obs_source_get_display_name(info.id);
	bool valid = rust == text && original == text && trace.calls == 2 && trace.last == &trace;
	text[0] = 'S';
	valid = valid && rust[0] == 'S' && original[0] == 'S';
	oracle_source_registry_exchange_core(previous);
	sti_active = previous_trace;
	return valid;
}
