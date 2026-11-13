#include "libobs/build/libobs.pch.h"

void obs_set_private_data(obs_data_t *settings)
{
	obs_data_clear(obs->data.private_data);
	if (settings)
		obs_data_apply(obs->data.private_data, settings);
}

obs_data_t *obs_get_private_data(void)
{
	obs_data_t *private_data = obs->data.private_data;
	obs_data_addref(private_data);
	return private_data;
}
