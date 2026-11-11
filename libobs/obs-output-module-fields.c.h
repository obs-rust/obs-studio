#include "libobs/build/libobs.pch.h"

obs_module_t *obs_output_module_next(obs_module_t *module);
size_t obs_output_module_count(obs_module_t *module);
const char *obs_output_module_id(obs_module_t *module, size_t index);
enum obs_module_load_state obs_output_module_state(obs_module_t *module);

/* Borrowed layout access only; Rust owns search policy, never module lifetime. */
obs_module_t *obs_output_module_next(obs_module_t *module)
{
	return module->next;
}

size_t obs_output_module_count(obs_module_t *module)
{
	return module->outputs.num;
}

const char *obs_output_module_id(obs_module_t *module, size_t index)
{
	return module->outputs.array[index];
}

enum obs_module_load_state obs_output_module_state(obs_module_t *module)
{
	return module->load_state;
}
