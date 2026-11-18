#include "libobs/build/libobs.pch.h"

struct obs_encoder_extra_data_fields obs_encoder_extra_data_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_extra_data_fields){encoder->info.get_extra_data, encoder->context.data};
}
