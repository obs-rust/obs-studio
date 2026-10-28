/* A module that reports a newer libobs than the one loading it, so that
 * obs_open_module() rejects it (MODULE_INCOMPATIBLE_VER) and
 * obs_load_core_modules() reports a failure. Stands in for a core module that
 * was built against a newer libobs while the rest of the installation is
 * older, in test_core_module_load.
 *
 * OBS_DECLARE_MODULE() cannot be used for that: it defines obs_module_ver() to
 * return this libobs's version. These are the same exports, with the version
 * bumped by one major. */
#include <obs-module.h>

static obs_module_t *obs_module_pointer;

MODULE_EXPORT void obs_module_set_pointer(obs_module_t *module);
void obs_module_set_pointer(obs_module_t *module)
{
	obs_module_pointer = module;
}

obs_module_t *obs_current_module(void)
{
	return obs_module_pointer;
}

MODULE_EXPORT uint32_t obs_module_ver(void);
uint32_t obs_module_ver(void)
{
	return LIBOBS_API_VER + 0x00010000;
}

MODULE_EXPORT bool obs_module_load(void);
bool obs_module_load(void)
{
	return true;
}
