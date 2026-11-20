#include "libobs/build/libobs.pch.h"

void obs_apply_private_data(obs_data_t *settings)
{
	if (!settings)
		return;

	obs_data_apply(obs->data.private_data, settings);
}
