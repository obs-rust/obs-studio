#include "plugins/frontend-tools/build/frontend-tools-c.pch.h"

OBS_DECLARE_MODULE()
OBS_MODULE_USE_DEFAULT_LOCALE("frontend-tools", "en-US")

void InitSceneSwitcher(void);
void FreeSceneSwitcher(void);

void InitOutputTimer(void);
void FreeOutputTimer(void);

#if defined(ENABLE_SCRIPTING)
void InitScripts(void);
void FreeScripts(void);
#endif

bool obs_module_load(void)
{
	frontend_tools_host_load();
	InitSceneSwitcher();
	InitOutputTimer();
#if defined(ENABLE_SCRIPTING)
	if (obs_frontend_is_safe_mode_enabled()) {
		blog(LOG_WARNING, "Scripting is disabled in Safe Mode");
	} else {
		InitScripts();
	}
#endif
	return true;
}

void obs_module_unload(void)
{
	frontend_tools_host_unload();
	FreeSceneSwitcher();
	FreeOutputTimer();
#if defined(ENABLE_SCRIPTING)
	if (!obs_frontend_is_safe_mode_enabled()) {
		FreeScripts();
	}
#endif
}
