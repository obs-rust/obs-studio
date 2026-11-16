#include "libobs/build/libobs.pch.h"

obs_module_t *obs_encoder_module_root(bool disabled);

/* Keep global core access separate from standalone field-adapter consumers. */
obs_module_t *obs_encoder_module_root(bool disabled)
{
	return disabled ? obs->first_disabled_module : obs->first_module;
}
