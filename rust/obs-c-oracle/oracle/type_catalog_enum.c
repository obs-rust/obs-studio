/* Original six bodies, real-header catalog fixtures and production adapters. */
#include "libobs/build/libobs.pch.h"
#include <string.h>
extern struct obs_core *core_texture_activity_oracle_core;
bool oracle_obs_enum_input_types(size_t index, const char **id);
bool oracle_obs_enum_filter_types(size_t index, const char **id);
bool oracle_obs_enum_transition_types(size_t index, const char **id);
bool oracle_obs_enum_output_types(size_t index, const char **id);
bool oracle_obs_enum_encoder_types(size_t index, const char **id);
bool oracle_obs_enum_service_types(size_t index, const char **id);
#define obs core_texture_activity_oracle_core
#define obs_enum_input_types oracle_obs_enum_input_types
#define obs_enum_filter_types oracle_obs_enum_filter_types
#define obs_enum_transition_types oracle_obs_enum_transition_types
#define obs_enum_output_types oracle_obs_enum_output_types
#define obs_enum_encoder_types oracle_obs_enum_encoder_types
#define obs_enum_service_types oracle_obs_enum_service_types
#include "libobs/obs-type-catalog-enum.c.h"
#undef obs_enum_input_types
#undef obs_enum_filter_types
#undef obs_enum_transition_types
#undef obs_enum_output_types
#undef obs_enum_encoder_types
#undef obs_enum_service_types
#undef obs
#include "type_catalog_enum_fields.inc"

typedef bool (*catalog_query)(size_t, const char **);
bool oracle_type_catalog_enum_check(const char *const *ids, const size_t *counts, size_t index,
				    const catalog_query *queries, const char *const *safe_ids,
				    const bool *safe_results);
bool oracle_type_catalog_enum_check(const char *const *ids, const size_t *counts, size_t index,
				    const catalog_query *queries, const char *const *safe_ids, const bool *safe_results)
{
	for (size_t c = 0; c < 6; ++c)
		if (counts[c] > 8)
			return false;
	struct obs_core core = {0}, before_core;
	struct obs_source_info input[8] = {0}, before_input[8];
	for (size_t i = 0; i < counts[0]; ++i)
		input[i].id = ids[0 * 8 + i];
	core.input_types.array = counts[0] ? input : NULL;
	core.input_types.num = counts[0];
	core.input_types.capacity = counts[0];
	memcpy(before_input, input, sizeof(input));
	struct obs_source_info filter[8] = {0}, before_filter[8];
	for (size_t i = 0; i < counts[1]; ++i)
		filter[i].id = ids[1 * 8 + i];
	core.filter_types.array = counts[1] ? filter : NULL;
	core.filter_types.num = counts[1];
	core.filter_types.capacity = counts[1];
	memcpy(before_filter, filter, sizeof(filter));
	struct obs_source_info transition[8] = {0}, before_transition[8];
	for (size_t i = 0; i < counts[2]; ++i)
		transition[i].id = ids[2 * 8 + i];
	core.transition_types.array = counts[2] ? transition : NULL;
	core.transition_types.num = counts[2];
	core.transition_types.capacity = counts[2];
	memcpy(before_transition, transition, sizeof(transition));
	struct obs_output_info output[8] = {0}, before_output[8];
	for (size_t i = 0; i < counts[3]; ++i)
		output[i].id = ids[3 * 8 + i];
	core.output_types.array = counts[3] ? output : NULL;
	core.output_types.num = counts[3];
	core.output_types.capacity = counts[3];
	memcpy(before_output, output, sizeof(output));
	struct obs_encoder_info encoder[8] = {0}, before_encoder[8];
	for (size_t i = 0; i < counts[4]; ++i)
		encoder[i].id = ids[4 * 8 + i];
	core.encoder_types.array = counts[4] ? encoder : NULL;
	core.encoder_types.num = counts[4];
	core.encoder_types.capacity = counts[4];
	memcpy(before_encoder, encoder, sizeof(encoder));
	struct obs_service_info service[8] = {0}, before_service[8];
	for (size_t i = 0; i < counts[5]; ++i)
		service[i].id = ids[5 * 8 + i];
	core.service_types.array = counts[5] ? service : NULL;
	core.service_types.num = counts[5];
	core.service_types.capacity = counts[5];
	memcpy(before_service, service, sizeof(service));
	memcpy(&before_core, &core, sizeof(core));
	struct obs_core *previous = core_texture_activity_oracle_core;
	core_texture_activity_oracle_core = &core;
	const catalog_query originals[] = {
		oracle_obs_enum_input_types,  oracle_obs_enum_filter_types,  oracle_obs_enum_transition_types,
		oracle_obs_enum_output_types, oracle_obs_enum_encoder_types, oracle_obs_enum_service_types,
	};
	bool valid = true;
	for (uint32_t c = 0; c < 6; ++c) {
		size_t count = SIZE_MAX;
		const void *array = obs_type_catalog_enum_registry_fields(&core, c, &count);
		valid = valid && count == counts[c];
		const void *arrays[] = {core.input_types.array,  core.filter_types.array,  core.transition_types.array,
					core.output_types.array, core.encoder_types.array, core.service_types.array};
		valid = valid && array == arrays[c];
		for (size_t i = 0; i < count; ++i) {
			const char *const *fields[] = {&input[i].id,  &filter[i].id,  &transition[i].id,
						       &output[i].id, &encoder[i].id, &service[i].id};
			valid = valid && obs_type_catalog_enum_id_fields(array, c, i) == fields[c];
		}
		const char *sentinel = ids[48 + c];
		const char *actual = sentinel, *original = sentinel;
		bool a = queries[c](index, &actual);
		bool b = originals[c](index, &original);
		valid = valid && a == b && a == safe_results[c] && actual == original && actual == safe_ids[c];
		if (index >= count)
			valid = valid && actual == sentinel && !queries[c](index, NULL) && !originals[c](index, NULL);
	}
	valid = valid && memcmp(&before_core, &core, sizeof(core)) == 0;
	valid = valid && memcmp(before_input, input, sizeof(input)) == 0;
	valid = valid && memcmp(before_filter, filter, sizeof(filter)) == 0;
	valid = valid && memcmp(before_transition, transition, sizeof(transition)) == 0;
	valid = valid && memcmp(before_output, output, sizeof(output)) == 0;
	valid = valid && memcmp(before_encoder, encoder, sizeof(encoder)) == 0;
	valid = valid && memcmp(before_service, service, sizeof(service)) == 0;
	core_texture_activity_oracle_core = previous;
	return valid;
}
