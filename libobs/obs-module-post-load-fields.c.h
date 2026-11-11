#include "libobs/build/libobs.pch.h"

/* Private field read only; ordering, NULL skipping and dispatch stay in Rust. */
typedef void (*obs_module_post_load_callback_t)(void);
obs_module_post_load_callback_t obs_module_post_load_callback(const obs_module_t *module);
obs_module_post_load_callback_t obs_module_post_load_callback(const obs_module_t *module)
{
	return module->post_load;
}
