/* Original traversal, production field adapters, and bounded real-header fixtures. */
#include "libobs/build/libobs.pch.h"
extern struct obs_core *core_texture_activity_oracle_core;
const char *oracle_obs_get_latest_input_type_id(const char *query);
#define obs core_texture_activity_oracle_core
#define obs_get_latest_input_type_id oracle_obs_get_latest_input_type_id
#include "latest-input-type-id.inc"
#undef obs_get_latest_input_type_id
#undef obs
#include "latest_input_type_id_fields.inc"

typedef const char *(*latest_query)(const char *);
bool oracle_latest_input_type_id_check(const char *const *ids, const char *const *names, const uint32_t *versions,
				       size_t count, const char *query, latest_query rust, size_t selected);
bool oracle_latest_input_type_id_check(const char *const *ids, const char *const *names, const uint32_t *versions,
				       size_t count, const char *query, latest_query rust, size_t selected)
{
	if (count > 8 || (query && selected >= count))
		return false; /* Missing non-NULL queries are outside the debug C domain. */
	struct obs_core core = {0}, before_core;
	struct obs_source_info entries[8] = {0}, before_entries[8];
	for (size_t i = 0; i < count; ++i) {
		entries[i].id = ids[i];
		entries[i].unversioned_id = names[i];
		entries[i].version = versions[i];
		entries[i].type = OBS_SOURCE_TYPE_FILTER;
	}
	core.source_types.array = count ? entries : NULL;
	core.source_types.num = count;
	core.source_types.capacity = count;
	memcpy(&before_core, &core, sizeof(core));
	memcpy(before_entries, entries, sizeof(entries));
	struct obs_core *previous = core_texture_activity_oracle_core;
	core_texture_activity_oracle_core = query ? &core : NULL;
	const char *actual = rust(query);
	const char *original = oracle_obs_get_latest_input_type_id(query);
	bool valid = actual == original && actual == (query ? ids[selected] : NULL);
	valid = valid && memcmp(&before_core, &core, sizeof(core)) == 0;
	valid = valid && memcmp(before_entries, entries, sizeof(entries)) == 0;
	core_texture_activity_oracle_core = previous;
	return valid;
}
