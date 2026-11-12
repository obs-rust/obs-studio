#include "libobs/build/libobs.pch.h"

const void *obs_source_type_enum_registry_fields(const struct obs_core *core, bool inputs, size_t *count);
const void *obs_source_type_enum_registry_fields(const struct obs_core *core, bool inputs, size_t *count)
{
	const obs_source_info_array_t *types = inputs ? &core->input_types : &core->source_types;
	*count = types->num;
	return types->array;
}

const char *const *obs_source_type_enum_id_fields(const void *array, size_t index, bool unversioned);
const char *const *obs_source_type_enum_id_fields(const void *array, size_t index, bool unversioned)
{
	const struct obs_source_info *info = (const struct obs_source_info *)array + index;
	return unversioned ? &info->unversioned_id : &info->id;
}
