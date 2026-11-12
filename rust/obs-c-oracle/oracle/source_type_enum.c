/* Original two exports and real production adapters over real-header registries.
 * The Rust test re-execs before borrowing the existing core-accessor fixture. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

extern struct obs_core *core_texture_activity_oracle_core;
bool oracle_obs_enum_source_types(size_t index, const char **id);
bool oracle_obs_enum_input_types2(size_t index, const char **id, const char **unversioned);
#define obs core_texture_activity_oracle_core
#define obs_enum_source_types oracle_obs_enum_source_types
#define obs_enum_input_types2 oracle_obs_enum_input_types2
#include "libobs/obs-source-type-enum.c.h"
#undef obs_enum_input_types2
#undef obs_enum_source_types
#undef obs
#include "source_type_enum_fields.inc"

typedef bool (*source_type_enum_query)(size_t, const char **);
typedef bool (*input_type_enum_query)(size_t, const char **, const char **);
bool oracle_source_type_enum_check(const char *const *source_ids, const char *const *input_ids,
				   const char *const *unversioned_ids, size_t source_count, size_t input_count,
				   size_t index, source_type_enum_query source_query,
				   input_type_enum_query input_query);
bool oracle_source_type_enum_check(const char *const *source_ids, const char *const *input_ids,
				   const char *const *unversioned_ids, size_t source_count, size_t input_count,
				   size_t index, source_type_enum_query source_query, input_type_enum_query input_query)
{
	if (source_count > 8 || input_count > 8)
		return false;
	struct obs_core core = {0}, before_core;
	struct obs_source_info sources[8] = {0}, inputs[8] = {0}, before_sources[8], before_inputs[8];
	for (size_t i = 0; i < source_count; ++i)
		sources[i].id = source_ids[i];
	for (size_t i = 0; i < input_count; ++i) {
		inputs[i].id = input_ids[i];
		inputs[i].unversioned_id = unversioned_ids[i];
	}
	core.source_types.array = source_count ? sources : NULL;
	core.source_types.num = source_count;
	core.source_types.capacity = source_count;
	core.input_types.array = input_count ? inputs : NULL;
	core.input_types.num = input_count;
	core.input_types.capacity = input_count;
	memcpy(&before_core, &core, sizeof(core));
	memcpy(before_sources, sources, sizeof(sources));
	memcpy(before_inputs, inputs, sizeof(inputs));
	struct obs_core *previous = core_texture_activity_oracle_core;
	core_texture_activity_oracle_core = &core;
	bool valid = true;
	for (unsigned input = 0; input < 2; ++input) {
		size_t count = SIZE_MAX;
		const void *array = obs_source_type_enum_registry_fields(&core, input != 0, &count);
		valid = valid && count == (input ? input_count : source_count) &&
			array == (input ? (const void *)core.input_types.array : (const void *)core.source_types.array);
		for (size_t i = 0; i < count; ++i) {
			valid = valid && obs_source_type_enum_id_fields(array, i, false) ==
						 (input ? &inputs[i].id : &sources[i].id);
			valid = valid && obs_source_type_enum_id_fields(array, i, true) ==
						 (input ? &inputs[i].unversioned_id : &sources[i].unversioned_id);
		}
	}
	const char sentinel[] = "untouched";
	const char *actual = sentinel, *expected = sentinel;
	bool result = source_query(index, &actual);
	bool original = oracle_obs_enum_source_types(index, &expected);
	valid = valid && result == original && actual == expected;
	if (index >= source_count)
		valid = valid && actual == sentinel && !source_query(index, NULL) &&
			!oracle_obs_enum_source_types(index, NULL);
	for (unsigned mask = 0; mask < 4; ++mask) {
		actual = expected = sentinel;
		const char *actual_unversioned = sentinel, *expected_unversioned = sentinel;
		result = input_query(index, mask & 1 ? &actual : NULL, mask & 2 ? &actual_unversioned : NULL);
		original = oracle_obs_enum_input_types2(index, mask & 1 ? &expected : NULL,
							mask & 2 ? &expected_unversioned : NULL);
		valid = valid && result == original && actual == expected && actual_unversioned == expected_unversioned;
		if (index >= input_count)
			valid = valid && actual == sentinel && actual_unversioned == sentinel;
	}
	actual = expected = sentinel;
	result = input_query(index, &actual, &actual);
	original = oracle_obs_enum_input_types2(index, &expected, &expected);
	valid = valid && result == original && actual == expected;
	valid = valid && memcmp(&core, &before_core, sizeof(core)) == 0 &&
		memcmp(sources, before_sources, sizeof(sources)) == 0 &&
		memcmp(inputs, before_inputs, sizeof(inputs)) == 0;
	core_texture_activity_oracle_core = previous;
	return valid;
}

void oracle_source_type_enum_layout(size_t *out);
void oracle_source_type_enum_layout(size_t *out)
{
	out[0] = sizeof(((struct obs_core *)0)->source_types.num);
	out[1] = _Alignof(size_t);
	out[2] = sizeof(((struct obs_source_info *)0)->id);
	out[3] = _Alignof(const char *);
	out[4] = sizeof(((struct obs_source_info *)0)->unversioned_id);
	out[5] = _Alignof(bool);
	out[6] = sizeof(bool);
}
