#include "libobs/build/libobs.pch.h"

/* Host extension selection remains in the existing host implementation. */
extern const char *get_module_extension(void);

void obs_find_modules2(obs_find_module_callback2_t callback, void *param)
{
	if (!obs)
		return;

	for (size_t i = 0; i < obs->module_paths.num; i++) {
		struct obs_module_path *omp = obs->module_paths.array + i;
		obs_module_paths_find(omp->bin, omp->data, omp->type, get_module_extension(),
				      obs_host_add_module_extension, callback, param);
	}
}

void obs_find_modules(obs_find_module_callback_t callback, void *param)
{
	/* the structure is ABI compatible so we can just cast the callback */
	obs_find_modules2((obs_find_module_callback2_t)callback, param);
}
