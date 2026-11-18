#include "libobs/build/libobs.pch.h"

bool obs_encoder_get_extra_data(const obs_encoder_t *encoder, uint8_t **extra_data, size_t *size)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_extra_data"))
		return false;

	if (encoder->info.get_extra_data && encoder->context.data)
		return encoder->info.get_extra_data(encoder->context.data, extra_data, size);

	return false;
}
