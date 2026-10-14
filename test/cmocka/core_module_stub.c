/* Minimal OBS module standing in for a core module in test_core_module_load. */
#include <obs-module.h>

OBS_DECLARE_MODULE()

#ifdef CORE_STUB_NEEDS_DEP
/* Defined in a shared library that is not installed next to the module, like
 * libcef.so for an obs-browser.so packaged without CEF. */
extern int core_stub_missing_dependency(void);
#endif

bool obs_module_load(void)
{
#ifdef CORE_STUB_NEEDS_DEP
	return core_stub_missing_dependency() == 1;
#else
	return true;
#endif
}
