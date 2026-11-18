#include "libobs/build/libobs.pch.h"

void obs_set_locale(const char *locale)
{
	struct obs_module *module;

	if (obs->locale)
		bfree(obs->locale);
	obs->locale = bstrdup(locale);

	module = obs->first_module;
	while (module) {
		if (module->set_locale)
			module->set_locale(locale);

		module = module->next;
	}
}
