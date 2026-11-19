#include "libobs/build/libobs.pch.h"

/* These adapters only borrow fields of real C layouts. Selection numbers are
 * private to the Rust shim; bounds and output writes stay in the safe core. */
const void *obs_type_catalog_enum_registry_fields(const struct obs_core *core, uint32_t catalog, size_t *count);
const void *obs_type_catalog_enum_registry_fields(const struct obs_core *core, uint32_t catalog, size_t *count)
{
	switch (catalog) {
	case 0:
		*count = core->input_types.num;
		return core->input_types.array;
	case 1:
		*count = core->filter_types.num;
		return core->filter_types.array;
	case 2:
		*count = core->transition_types.num;
		return core->transition_types.array;
	case 3:
		*count = core->output_types.num;
		return core->output_types.array;
	case 4:
		*count = core->encoder_types.num;
		return core->encoder_types.array;
	case 5:
		*count = core->service_types.num;
		return core->service_types.array;
	default:
		abort();
	}
}

const char *const *obs_type_catalog_enum_id_fields(const void *array, uint32_t catalog, size_t index);
const char *const *obs_type_catalog_enum_id_fields(const void *array, uint32_t catalog, size_t index)
{
	switch (catalog) {
	case 0:
	case 1:
	case 2:
		return &((const struct obs_source_info *)array)[index].id;
	case 3:
		return &((const struct obs_output_info *)array)[index].id;
	case 4:
		return &((const struct obs_encoder_info *)array)[index].id;
	case 5:
		return &((const struct obs_service_info *)array)[index].id;
	default:
		abort();
	}
}
