#include "libobs/build/libobs.pch.h"

void obs_post_load_modules(void)
{
	obs_host_post_load_modules();
	for (obs_module_t *mod = obs->first_module; !!mod; mod = mod->next)
		if (mod->post_load)
			mod->post_load();
}
