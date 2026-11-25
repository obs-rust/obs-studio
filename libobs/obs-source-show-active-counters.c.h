#include "libobs/build/libobs.pch.h"

void obs_source_inc_showing(obs_source_t *source)
{
	if (obs_source_valid(source, "obs_source_inc_showing"))
		obs_source_activate(source, AUX_VIEW);
}

void obs_source_inc_active(obs_source_t *source)
{
	if (obs_source_valid(source, "obs_source_inc_active"))
		obs_source_activate(source, MAIN_VIEW);
}

void obs_source_dec_showing(obs_source_t *source)
{
	if (obs_source_valid(source, "obs_source_dec_showing"))
		obs_source_deactivate(source, AUX_VIEW);
}

void obs_source_dec_active(obs_source_t *source)
{
	if (obs_source_valid(source, "obs_source_dec_active"))
		obs_source_deactivate(source, MAIN_VIEW);
}
