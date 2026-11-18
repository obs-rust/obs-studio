#include "libobs/build/libobs.pch.h"

struct obs_core *obs_core_set_locale_core(void);
struct obs_core *obs_core_set_locale_core(void)
{
	return obs;
}

char **obs_core_set_locale_field(struct obs_core *core);
char **obs_core_set_locale_field(struct obs_core *core)
{
	return &core->locale;
}

struct obs_module *obs_core_set_locale_first(struct obs_core *core);
struct obs_module *obs_core_set_locale_first(struct obs_core *core)
{
	return core->first_module;
}

void (*obs_core_set_locale_callback(struct obs_module *module))(const char *);
void (*obs_core_set_locale_callback(struct obs_module *module))(const char *)
{
	return module->set_locale;
}

struct obs_module *obs_core_set_locale_next(struct obs_module *module);
struct obs_module *obs_core_set_locale_next(struct obs_module *module)
{
	return module->next;
}
