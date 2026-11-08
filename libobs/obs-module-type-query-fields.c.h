#include "libobs/build/libobs.pch.h"

const enum obs_runtime_module_type *obs_module_type_query_field(const obs_module_t *module);
const enum obs_runtime_module_type *obs_module_type_query_field(const obs_module_t *module)
{
	return module ? &module->module_type : NULL;
}
