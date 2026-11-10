#include "libobs/build/libobs.pch.h"

struct obs_module_metadata **obs_module_load_metadata_slot(struct obs_module *module);
struct obs_module_metadata **obs_module_load_metadata_slot(struct obs_module *module)
{
	return &module->metadata;
}
