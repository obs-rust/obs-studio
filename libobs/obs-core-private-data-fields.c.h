#include "libobs/build/libobs.pch.h"

obs_data_t *const *obs_core_private_data_field(void)
{
	return &obs->data.private_data;
}
